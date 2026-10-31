/**
 * Runs the browser LSP worker over files captured by the extension's workspace provider.
 * Configurations and `@plugin` packages are bundled from that inventory and evaluated as ESM.
 * Node imports fail resolution. Snapshot, compile, and sort requests require one workspace.
 * Reads and compile requests also require matching document and reload revisions.
 */

import { BrowserAnalysisWorkspace } from "@stillwind/analysis/browser";
import {
  ConfigurationLoadError,
  documentConfiguration,
  loadBrowserConfig,
} from "@stillwind/config/browser";
import { virtualPath } from "@stillwind/host/browser";
import { createLanguageServer } from "@stillwind/language-server";
import * as paths from "pathe";
import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  ProposedFeatures,
} from "vscode-languageserver/browser";

import type { BundledModule } from "./bundle.ts";
import { consumeWorkspaceFiles, workspaceFileEntries } from "./inventory.ts";
import { installLibraries } from "./libraries.ts";
import { browserPluginHost } from "./plugins.ts";
import { selectWorkspace } from "./select-workspace.ts";

const connection = createConnection(
  ProposedFeatures.all,
  new BrowserMessageReader(self),
  new BrowserMessageWriter(self),
);

/** Opened workspaces, keyed by configuration identity. */
let workspaces = new Map<string, BrowserAnalysisWorkspace>();
let initial = true;
/** Counts workspace openings so compile requests can reject snapshots captured before a reload. */
let generation = 0;
let libraries: Promise<[string, string][]> | undefined;
/** Files of the latest workspace opening, which discovery searches for configurations. */
let inventory: Map<string, string> | undefined;
/** Explicit configuration path of the latest workspace opening. It disables discovery. */
let explicitConfig: string | undefined;
/** Error messages of configurations that failed to open, keyed by file or workspace folder. */
let failures = new Map<string, string>();

/**
 * Bundles a module of the captured inventory. The bundler loads on first use.
 *
 * @param files - Virtual absolute paths and text.
 * @param entry - Module path present in `files`.
 * @returns One ESM source and the files that it contains.
 * @throws If an import is unavailable, uses a Node builtin, or fails compilation.
 */
async function bundle(files: ReadonlyMap<string, string>, entry: string): Promise<BundledModule> {
  return (await import("./bundle.ts")).bundleModule(files, entry);
}

/**
 * Evaluates an ESM source through a Blob URL.
 *
 * @param code - Bundled module source.
 * @returns The module namespace.
 * @throws If evaluation fails.
 */
async function evaluate(code: string): Promise<unknown> {
  const url = URL.createObjectURL(
    new Blob([code], {
      type: "text/javascript",
    }),
  );
  try {
    return await import(/* @vite-ignore */ url);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * Evaluates a configuration from the captured inventory and opens its service.
 *
 * @param directory - Virtual directory where discovery starts.
 * @param rootUri - Workspace folder URI that supplies the document URI scheme and authority.
 * @param files - Captured inventory.
 * @param configFile - Explicit configuration path, or omission for upward discovery.
 * @returns The configuration identity and service, or undefined when discovery finds no file.
 * @throws If evaluation fails or the configuration is invalid.
 */
async function openWorkspace(
  directory: string,
  rootUri: string,
  files: Map<string, string>,
  configFile?: string,
): Promise<[string, BrowserAnalysisWorkspace] | undefined> {
  const config = await loadBrowserConfig(
    directory,
    {
      exists: (id) => files.has(id),
      list: () => files.keys(),
      evaluate: async (id) =>
        (
          (await evaluate((await bundle(files, id)).code)) as {
            default?: unknown;
          }
        ).default,
    },
    configFile,
  );
  if (!config) {
    return undefined;
  }
  const service = new BrowserAnalysisWorkspace({
    config: config.configFile ?? rootUri,
    projects: config.projects,
    files,
    plugins: browserPluginHost(files, bundle, evaluate),
    documentUri: (id) => {
      const uri = new URL(rootUri);
      uri.pathname = id;
      return uri.href;
    },
  });
  workspaces.set(config.configFile ?? rootUri, service);
  return [config.configFile ?? rootUri, service];
}

/**
 * Selects the identity that reports a failure to open a configuration.
 *
 * @param error - Failure from opening a configuration.
 * @param fallback - Identity to use when the failure names no configuration file.
 * @returns The path of the file that failed, or fallback.
 */
function failureId(error: unknown, fallback: string): string {
  return error instanceof ConfigurationLoadError ? error.configFile : fallback;
}

const server = createLanguageServer(connection, {
  configurationUri: (id, folders) => {
    const rootUri = folders.find((folder) => id.startsWith(`${virtualPath(folder)}/`));
    if (!rootUri) {
      return undefined;
    }
    const uri = new URL(rootUri);
    uri.pathname = id;
    return uri.href;
  },
  discoverWorkspace: async (uri, { workspaceFolders }, known) => {
    const files = inventory;
    if (!files || explicitConfig) {
      return undefined;
    }
    const id = virtualPath(uri);
    const config = documentConfiguration(
      id,
      workspaceFolders.map(virtualPath),
      (file) => files.has(file),
      paths,
    );
    const rootUri = workspaceFolders.find((folder) => id.startsWith(`${virtualPath(folder)}/`));
    if (!config || !rootUri || known.has(config)) {
      return undefined;
    }
    try {
      const opened = await openWorkspace(paths.dirname(config), rootUri, files);
      return {
        workspaces: new Map(opened ? [opened] : []),
      };
    } catch (error) {
      const id = failureId(error, config);
      failures.set(id, String(error));
      return {
        workspaces: new Map(),
        errors: new Map([[id, String(error)]]),
      };
    }
  },
  openWorkspaces: async ({ workspaceFolders, initializationOptions }) => {
    generation++;
    const captured = initial
      ? (initializationOptions as {
          /** Original document URI and captured text for each file, including open-buffer overrides. */
          files: [string, string][];
          /** Optional explicit configuration path passed to discovery for each workspace root. */
          configFile?: string;
        })
      : await connection.sendRequest<{
          /** Original document URI and freshly captured text for each file. */
          files: [string, string][];
          /** Optional configuration path from the current extension setting. */
          configFile?: string;
        }>("stillwind/readWorkspace", {
          roots: workspaceFolders,
        });
    initial = false;
    libraries ??= fetch(new URL("./libraries.json", import.meta.url)).then(async (response) => {
      if (!response.ok) {
        throw new Error("Cannot load bundled TypeScript standard libraries");
      }
      return (await response.json()) as [string, string][];
    });
    const files = consumeWorkspaceFiles(captured);
    installLibraries(files, await libraries, workspaceFolders.map(virtualPath));
    inventory = files;
    explicitConfig = captured.configFile;
    workspaces = new Map();
    failures = new Map();
    const opened = new Map<string, BrowserAnalysisWorkspace>();
    for (const rootUri of workspaceFolders) {
      try {
        const workspace = await openWorkspace(
          virtualPath(rootUri),
          rootUri,
          files,
          captured.configFile,
        );
        if (workspace) {
          opened.set(...workspace);
        }
      } catch (error) {
        failures.set(failureId(error, rootUri), String(error));
      }
    }
    return {
      workspaces: opened,
      errors: new Map(failures),
    };
  },
  onExit: () => {
    self.close();
  },
});

connection.onRequest(
  "stillwind/snapshot",
  async (input?: {
    /** Configuration identity of the workspace to capture. Omission requires one open workspace. */
    config?: string;
    /** Whether to recapture workspace files and configuration before returning the snapshot. */
    reload?: boolean;
    /** Whether only package manifests include text; other file entries contain null. */
    metadataOnly?: boolean;
  }) => {
    if (input?.reload) {
      await server.reload();
    }
    await server.ready();
    const [config, workspace] = selectWorkspace(
      workspaces,
      failures,
      "Application build",
      input?.config,
    );
    return {
      config,
      generation,
      revision: workspace.revision,
      files: workspaceFileEntries(workspace.files(), input?.metadataOnly),
    };
  },
);

connection.onRequest(
  "stillwind/readFile",
  async (input: {
    /** Configuration identity returned by stillwind/snapshot. Omission requires one open workspace. */
    config?: string;
    /** Original URI of a file listed by stillwind/snapshot. */
    uri: string;
    /** Document revision captured by stillwind/snapshot. */
    revision: number;
    /** Workspace opening generation captured by stillwind/snapshot. */
    generation: number;
  }) => {
    await server.ready();
    if (input.generation !== generation) {
      throw new Error("Obsolete workspace snapshot");
    }
    const [, workspace] = selectWorkspace(workspaces, failures, "Application build", input.config);
    if (input.revision !== workspace.revision) {
      throw new Error("Obsolete workspace revision");
    }
    const text = workspace.readFile(input.uri, input.revision);
    if (text === undefined) {
      throw new Error(`Missing virtual file ${input.uri}`);
    }
    return text;
  },
);

connection.onRequest(
  "stillwind/compile",
  async (input: {
    /** Configuration identity returned by stillwind/snapshot. Omission requires one open workspace. */
    config?: string;
    /** Original document URI to compile within the selected workspace. */
    uri: string;
    /** Workspace revision captured by stillwind/snapshot, including open-buffer updates. */
    revision: number;
    /** Opening generation captured by stillwind/snapshot, which must match the current worker. */
    generation: number;
  }) => {
    await server.ready();
    if (input.generation !== generation) {
      throw new Error("Obsolete workspace snapshot");
    }
    const [, workspace] = selectWorkspace(workspaces, failures, "Application build", input.config);
    return workspace.compile(input.uri, input.revision);
  },
);

connection.onRequest(
  "stillwind/sort",
  async (input: {
    /** Original document URI. Every open workspace whose projects include it answers. */
    uri: string;
    /** Text that the caller formats. It must equal the server's current buffer for the URI. */
    source: string;
    /** Half-open UTF-16 ranges in `source` that sorting must leave untouched. */
    protectedRanges?: {
      start: number;
      end: number;
    }[];
  }) => {
    await server.ready();
    if (!workspaces.size) {
      selectWorkspace(workspaces, failures, "Sorting");
    }
    const entries = [];
    for (const workspace of workspaces.values()) {
      if (workspace.files().get(virtualPath(input.uri)) !== input.source) {
        throw new Error("Obsolete document buffer");
      }
      entries.push(
        ...(await workspace.query(input.uri, {
          kind: "sort",
          protectedRanges: input.protectedRanges,
        })),
      );
    }
    const answers = new Set(entries.map((entry) => JSON.stringify(entry.result.edits ?? [])));
    if (answers.size > 1) {
      throw new Error("Stillwind projects disagree on the sort order of this document");
    }
    return {
      edits: entries[0]?.result.edits ?? [],
    };
  },
);

connection.listen();
