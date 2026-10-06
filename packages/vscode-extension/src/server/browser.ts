import { BrowserAnalysisWorkspace } from "@stillwind/analysis/browser";
import { loadBrowserConfig } from "@stillwind/config/browser";
import { resolveVirtualModule, virtualPath } from "@stillwind/host/browser";
import { createLanguageServer } from "@stillwind/language-server";
import * as esbuild from "esbuild-wasm";
import {
  BrowserMessageReader,
  BrowserMessageWriter,
  createConnection,
  ProposedFeatures,
} from "vscode-languageserver/browser";

import { installLibraries } from "./libraries.ts";

const connection = createConnection(
  ProposedFeatures.all,
  new BrowserMessageReader(self),
  new BrowserMessageWriter(self),
);

let workspaces: BrowserAnalysisWorkspace[] = [];
let initialization: Promise<void> | undefined;
let initial = true;
let generation = 0;
let libraries: Promise<[string, string][]> | undefined;

const server = createLanguageServer(connection, {
  openWorkspaces: async ({ workspaceFolders, initializationOptions }) => {
    generation++;
    const captured = initial
      ? (initializationOptions as {
          files: [string, string][];
          configFile?: string;
        })
      : await connection.sendRequest<{
          files: [string, string][];
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
            evaluate: async (id) => {
              initialization ??= esbuild.initialize({
                wasmURL: new URL("./esbuild.wasm", import.meta.url).href,
                worker: false,
              });
              await initialization;
              const result = await esbuild.build({
                entryPoints: [id],
                bundle: true,
                write: false,
                format: "esm",
                platform: "browser",
                target: "esnext",
                plugins: [
                  {
                    name: "virtual-configuration",
                    setup(build) {
                      build.onResolve({ filter: /.*/ }, (args) => {
                        if (args.path.startsWith("node:")) {
                          throw new Error(
                            `Node module is unavailable in browser configuration: ${args.path}`,
                          );
                        }
                        const file =
                          args.kind === "entry-point"
                            ? args.path
                            : resolveVirtualModule(files, args.path, args.importer);
                        if (!file) {
                          throw new Error(`Cannot resolve configuration import ${args.path}`);
                        }
                        return {
                          path: file,
                          namespace: "workspace",
                        };
                      });
                      build.onLoad(
                        {
                          filter: /.*/,
                          namespace: "workspace",
                        },
                        (args) => ({
                          contents: files.get(args.path),
                          loader: args.path.endsWith(".json") ? "json" : "ts",
                        }),
                      );
                    },
                  },
                ],
              });
              const url = URL.createObjectURL(
                new Blob([result.outputFiles![0].text], {
                  type: "text/javascript",
                }),
              );
              try {
                return (await import(/* @vite-ignore */ url)).default as unknown;
              } finally {
                URL.revokeObjectURL(url);
              }
            },
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

connection.onRequest("stillwind/snapshot", async (input?: { reload?: boolean }) => {
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
});

connection.onRequest(
  "stillwind/compile",
  async (input: { uri: string; revision: number; generation: number }) => {
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

connection.listen();
