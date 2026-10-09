import * as Babel from "@babel/standalone";
import type {
  BrowserModuleAnalysis,
  BrowserModuleCss,
  BrowserStylesheetOutput,
} from "@stillwind/host/browser";
import { resolveVirtualModule } from "@stillwind/host/resolution";
import * as esbuild from "esbuild-wasm";
import { resolve } from "pathe";

const replies = new Map<
  number,
  {
    resolve(value: Compilation): void;
    reject(error: Error): void;
  }
>();
let sequence = 0;
let initialization: Promise<void> | undefined;

/** Stillwind output sent from the extension's LSP worker for one captured source. */
interface Compilation {
  graph: BrowserModuleAnalysis;
  css: string;
  rejectedNames: BrowserStylesheetOutput["rejectedNames"];
  cssValues: [string, BrowserModuleCss][];
}

self.onmessage = async ({
  data,
}: MessageEvent<{
  id: number;
  kind: "build" | "compile-reply";
  revision: number;
  generation: number;
  files: [string, string][];
  value?: Compilation;
  error?: string;
}>) => {
  if (data.kind === "compile-reply") {
    const reply = replies.get(data.id);
    replies.delete(data.id);
    if (data.error) {
      reply?.reject(new Error(data.error));
    } else {
      reply?.resolve(data.value!);
    }
    return;
  }

  try {
    initialization ??= Promise.all([
      esbuild.initialize({
        wasmURL: new URL(`${import.meta.env.BASE_URL}esbuild.wasm`, self.location.origin).href,
        worker: false,
      }),
      import(
        /* @vite-ignore */ new URL(
          `${import.meta.env.BASE_URL}react-refresh-babel.js`,
          self.location.origin,
        ).href
      ).then((module) => {
        Babel.registerPlugin("react-refresh", module.default);
      }),
    ]).then(() => {});
    await initialization;
    const files = new Map(data.files);
    const transformed = new Map<string, string>();
    const cssValues = new Map<string, Compilation["cssValues"][number][1]>();
    const rejectedNames = new Map<string, BrowserStylesheetOutput["rejectedNames"][number]>();
    let css = "";
    const output = await esbuild.build({
      entryPoints: ["/workspace/Component.tsx"],
      bundle: true,
      write: false,
      outdir: "/build",
      format: "iife",
      globalName: "StillwindApplication",
      platform: "browser",
      jsx: "automatic",
      sourcemap: "inline",
      sourcesContent: true,
      define: {
        "process.env.NODE_ENV": '"development"',
      },
      plugins: [
        {
          name: "browser-workspace",
          setup(build) {
            build.onLoad({ filter: /.*/, namespace: "css-value" }, (args) => {
              const value = cssValues.get(args.path);
              if (!value) {
                throw new Error(`Missing compiled CSS value for ${args.path}`);
              }
              return {
                contents: `export const parts = ${JSON.stringify(value.parts)}; export const css = ${JSON.stringify(value.css)}; export default css;`,
                loader: "js",
              };
            });
            build.onResolve(
              {
                filter: /.*/,
              },
              (args) => {
                if (args.path === "stillwind/module-css") {
                  return { path: args.importer, namespace: "css-value" };
                }
                if (/^(react|react-dom)(\/|$)/.test(args.path)) {
                  return {
                    path: args.path,
                    external: true,
                  };
                }
                const file =
                  args.kind === "entry-point"
                    ? args.path
                    : resolveVirtualModule(files, args.path, args.importer);
                if (!file) {
                  throw new Error(`Cannot resolve ${args.path} from ${args.importer}`);
                }
                return {
                  path: file,
                  namespace: "workspace",
                };
              },
            );
            build.onLoad(
              {
                filter: /.*/,
                namespace: "workspace",
              },
              async (args) => {
                let code = files.get(args.path);
                if (code === undefined) {
                  throw new Error(`Missing virtual file ${args.path}`);
                }
                if (!args.path.includes("/node_modules/") && /\.[cm]?[jt]sx?$/.test(args.path)) {
                  if (!transformed.has(args.path)) {
                    const id = ++sequence;
                    const compilation = await new Promise<Compilation>((resolve, reject) => {
                      replies.set(id, { resolve, reject });
                      self.postMessage({
                        kind: "compile",
                        id,
                        uri: `file://${args.path}`,
                        revision: data.revision,
                        generation: data.generation,
                      });
                    });
                    if (compilation.graph.generation === "blocked") {
                      throw new Error(
                        compilation.graph.batches
                          .flatMap((batch) => batch.reports.map((report) => report.message))
                          .join("\n") || "Stillwind code generation is blocked",
                      );
                    }
                    for (const module of compilation.graph.modules) {
                      if (module.code !== null) {
                        transformed.set(resolve("/workspace", module.moduleId), module.code);
                      }
                    }
                    css = compilation.css;
                    for (const entry of [
                      ...compilation.rejectedNames,
                      ...compilation.graph.modules.flatMap((module) => module.rejectedNames),
                      ...compilation.cssValues.flatMap(([, value]) => value.rejectedNames),
                    ]) {
                      rejectedNames.set(
                        JSON.stringify([entry.kind, entry.hash, entry.reason]),
                        entry,
                      );
                    }
                    for (const [file, value] of compilation.cssValues) {
                      cssValues.set(file, value);
                    }
                  }
                  code = transformed.get(args.path) ?? code;
                  const refreshCode = Babel.transform(code, {
                    filename: args.path,
                    sourceType: "module",
                    presets: [
                      [
                        "typescript",
                        {
                          allExtensions: true,
                          isTSX: args.path.endsWith("x"),
                        },
                      ],
                    ],
                    plugins: [
                      [
                        "react-refresh",
                        {
                          skipEnvCheck: true,
                          emitFullSignatures: true,
                        },
                      ],
                    ],
                  }).code;
                  code = `const $RefreshReg$ = (type, name) => window.$RefreshReg$(type, ${JSON.stringify(args.path + " ")} + name);\nconst $RefreshSig$ = window.$RefreshSig$;\n${refreshCode}`;
                }
                return {
                  contents: code,
                  loader: args.path.endsWith(".json")
                    ? "json"
                    : args.path.endsWith(".css")
                      ? "css"
                      : args.path.endsWith("x")
                        ? "jsx"
                        : "js",
                  resolveDir: "/workspace",
                };
              },
            );
          },
        },
      ],
    });
    const code =
      output.outputFiles!.find((file) => file.path.endsWith(".js"))?.text ??
      output.outputFiles![0].text;
    self.postMessage({
      kind: "built",
      id: data.id,
      code,
      css:
        css +
        output
          .outputFiles!.filter((file) => file.path.endsWith(".css"))
          .map((file) => file.text)
          .join("\n"),
      transformed: [...transformed],
      rejectedNames: [...rejectedNames.values()],
    });
  } catch (error) {
    self.postMessage({
      kind: "failed",
      id: data.id,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
