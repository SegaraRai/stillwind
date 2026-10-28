/**
 * Runs the browser LSP worker over files captured by the extension's workspace provider.
 * Configurations and `@plugin` packages are bundled from that inventory and evaluated as ESM.
 * Node imports fail resolution. Snapshot, compile, and sort requests require one workspace.
 * Compile requests also require matching document and reload revisions.
 */

import { BrowserAnalysisWorkspace } from "@stillwind/analysis/browser";
import { loadBrowserConfig } from "@stillwind/config/browser";
import { virtualPath } from "@stillwind/host/browser";
import { createLanguageServer } from "@stillwind/language-server";
import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  ProposedFeatures,
} from "vscode-languageserver/browser";

import type { BundledModule } from "./bundle.ts";
import { installLibraries } from "./libraries.ts";
import { browserPluginHost } from "./plugins.ts";

const connection = createConnection(
  ProposedFeatures.all,
  new BrowserMessageReader(self),
  new BrowserMessageWriter(self),
);

let workspaces: BrowserAnalysisWorkspace[] = [];
let initial = true;
/** Counts workspace openings so compile requests can reject snapshots captured before a reload. */
let generation = 0;
let libraries: Promise<[string, string][]> | undefined;

/**
 * Bundles a module of the captured inventory. The bundler loads on first use.
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

const server = createLanguageServer(connection, {
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
    const files = new Map(captured.files.map(([uri, text]) => [virtualPath(uri), text] as const));
    installLibraries(files, await libraries, workspaceFolders.map(virtualPath));
    workspaces = [];
    const opened = new Map<string, BrowserAnalysisWorkspace>();
    const errors = new Map<string, string>();
    for (const rootUri of workspaceFolders) {
      const root = virtualPath(rootUri);
      try {
        const config = await loadBrowserConfig(
          root,
          {
            exists: (id) => files.has(id),
            evaluate: async (id) =>
              (
                (await evaluate((await bundle(files, id)).code)) as {
                  default?: unknown;
                }
              ).default,
          },
          captured.configFile,
        );
        if (!config) {
          throw new Error(`Cannot find Stillwind configuration in ${rootUri}`);
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
        workspaces.push(service);
        opened.set(config.configFile ?? rootUri, service);
      } catch (error) {
        errors.set(rootUri, String(error));
      }
    }
    return {
      workspaces: opened,
      errors,
    };
  },
  onExit: () => {
    self.close();
  },
});

connection.onRequest(
  "stillwind/snapshot",
  async (input?: {
    /** Whether to recapture workspace files and configuration before returning the snapshot. */
    reload?: boolean;
  }) => {
    if (input?.reload) {
      await server.reload();
    }
    await server.ready();
    if (workspaces.length !== 1) {
      throw new Error("Application builds require exactly one opened workspace");
    }
    const workspace = workspaces[0];
    return {
      generation,
      revision: workspace.revision,
      files: [...workspace.files()],
    };
  },
);

connection.onRequest(
  "stillwind/compile",
  async (input: {
    /** Original document URI to compile within the sole opened workspace. */
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
    if (workspaces.length !== 1) {
      throw new Error("Application builds require exactly one opened workspace");
    }
    return workspaces[0].compile(input.uri, input.revision);
  },
);

connection.onRequest(
  "stillwind/sort",
  async (input: {
    /** Original document URI within the sole opened workspace. */
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
    if (workspaces.length !== 1) {
      throw new Error("Sorting requires exactly one opened workspace");
    }
    const workspace = workspaces[0];
    if (workspace.files().get(virtualPath(input.uri)) !== input.source) {
      throw new Error("Obsolete document buffer");
    }
    const entries = await workspace.query(input.uri, {
      kind: "sort",
      protectedRanges: input.protectedRanges,
    });
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
