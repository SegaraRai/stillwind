import type { Plugin } from "@rolldown/browser";
import type {
  BrowserModuleAnalysis,
  BrowserModuleCss,
  BrowserStylesheetOutput,
} from "@stillwind/host/browser";
import { resolve } from "pathe";

import { BuildFiles } from "./files.ts";
import { inlineImports } from "./stylesheet.ts";

const rolldownBinding = import("@rolldown/browser");

const replies = new Map<
  number,
  {
    resolve(value: unknown): void;
    reject(error: Error): void;
  }
>();

let sequence = 0;

/**
 * Stillwind output sent from the extension's LSP worker for one captured source.
 */
interface Compilation {
  graph: BrowserModuleAnalysis;
  css: string;
  rejectedNames: BrowserStylesheetOutput["rejectedNames"];
  cssValues: [string, BrowserModuleCss][];
}

/** Prefix of module IDs that read a runtime module through the preview frame's `require`. */
const EXTERNAL = "\0external:";

/** Prefix of module IDs that export the CSS of a `stillwind/module-css` import. */
const CSS_VALUE = "\0css-value:";

self.onmessage = async ({
  data,
}: MessageEvent<{
  id: number;
  kind: "build" | "compile-reply" | "read-reply";
  revision: number;
  generation: number;
  files: [string, string | null][];
  value?: unknown;
  error?: string;
}>) => {
  if (data.kind !== "build") {
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
    // WASM initialization yields to incoming messages. The listener must already exist at that point.
    const { rolldown } = await rolldownBinding;

    const transformed = new Map<string, string>();
    const moduleCss = new Map<string, string>();
    const cssValues = new Map<string, Compilation["cssValues"][number][1]>();
    const rejectedNames = new Map<string, BrowserStylesheetOutput["rejectedNames"][number]>();
    const stylesheets = new Map<string, string>();

    let css = "";

    const request = <T>(kind: "compile" | "read", path: string) => {
      const id = ++sequence;
      return new Promise<T>((resolve, reject) => {
        replies.set(id, {
          resolve: (value) => resolve(value as T),
          reject,
        });
        self.postMessage({
          kind,
          id,
          uri: `file://${path.split("/").map(encodeURIComponent).join("/")}`,
          revision: data.revision,
          generation: data.generation,
        });
      });
    };
    const compile = (path: string) => request<Compilation>("compile", path);
    const files = new BuildFiles(data.files, (path) => request<string>("read", path));

    const workspace: Plugin = {
      name: "browser-workspace",
      resolveId(source, importer) {
        if (source === "stillwind/module-css" && importer) {
          return CSS_VALUE + importer;
        }
        // React must be the instance that the preview frame's renderer and React Refresh use.
        if (/^(react|react-dom)(\/|$)/.test(source)) {
          return EXTERNAL + source;
        }
        const file = importer ? files.resolve(source, importer) : source;
        if (!file) {
          throw new Error(`Cannot resolve ${source} from ${importer}`);
        }
        return file;
      },
      async load(id) {
        if (id.startsWith(EXTERNAL)) {
          return {
            code: `module.exports = window.require(${JSON.stringify(id.slice(EXTERNAL.length))});`,
            moduleType: "js",
          };
        }
        if (id.startsWith(CSS_VALUE)) {
          const value = cssValues.get(id.slice(CSS_VALUE.length));
          if (!value) {
            throw new Error(`Missing compiled CSS value for ${id.slice(CSS_VALUE.length)}`);
          }
          return {
            code: `export const parts = ${JSON.stringify(value.parts)}; export const css = ${JSON.stringify(value.css)}; export default css;`,
            moduleType: "js",
          };
        }
        if (id.endsWith(".css")) {
          // Rolldown does not bundle CSS. The build concatenates imported stylesheets in module
          // order after bundling, so the module itself exports nothing.
          stylesheets.set(id, await inlineImports(files, id, new Set()));
          return {
            code: "",
            moduleType: "js",
            moduleSideEffects: "no-treeshake",
          };
        }
        const code = await files.read(id);
        if (id.endsWith(".json")) {
          return {
            code,
            moduleType: "json",
          };
        }
        const moduleType = /\.[cm]?tsx$/.test(id)
          ? "tsx"
          : /\.[cm]?ts$/.test(id)
            ? "ts"
            : id.endsWith("x")
              ? "jsx"
              : "js";
        if (id.includes("/node_modules/") || !/\.[cm]?[jt]sx?$/.test(id)) {
          return {
            code,
            moduleType,
          };
        }
        if (!transformed.has(id)) {
          const compilation = await compile(id);
          if (compilation.graph.generation === "blocked") {
            throw new Error(
              compilation.graph.batches
                .flatMap((batch) => batch.reports.map((report) => report.message))
                .join("\n") || "Stillwind code generation is blocked",
            );
          }
          for (const module of compilation.graph.modules) {
            const file = resolve("/workspace", module.moduleId);
            if (module.code !== null) {
              transformed.set(file, module.code);
            }
            if (!moduleCss.has(file)) {
              moduleCss.set(file, module.css);
            }
          }
          css = compilation.css;
          for (const entry of [
            ...compilation.rejectedNames,
            ...compilation.graph.modules.flatMap((module) => module.rejectedNames),
            ...compilation.cssValues.flatMap(([, value]) => value.rejectedNames),
          ]) {
            rejectedNames.set(JSON.stringify([entry.kind, entry.hash, entry.reason]), entry);
          }
          for (const [file, value] of compilation.cssValues) {
            cssValues.set(file, value);
          }
        }
        // The JSX transform adds React Refresh registrations. They go through per-module bindings
        // so that component IDs include the file path. The transform can call the bindings before
        // the module's first statement, so they are hoisted function declarations.
        return {
          code: `function $RefreshReg$(type, name) { window.$RefreshReg$(type, ${JSON.stringify(`${id} `)} + name); }\nfunction $RefreshSig$() { return window.$RefreshSig$(); }\n${transformed.get(id) ?? code}`,
          moduleType,
        };
      },
    };

    const bundle = await rolldown({
      input: "/workspace/App.tsx",
      cwd: "/",
      platform: "browser",
      tsconfig: false,
      transform: {
        jsx: {
          runtime: "automatic",
          development: true,
          refresh: true,
        },
        define: {
          "process.env.NODE_ENV": '"development"',
        },
      },
      plugins: [workspace],
    });
    let code: string;
    let order: string[];
    try {
      const output = await bundle.generate({
        format: "iife",
        name: "StillwindApplication",
        sourcemap: "inline",
        codeSplitting: false,
      });
      const chunk = output.output.find((item) => item.type === "chunk" && item.isEntry);
      if (chunk?.type !== "chunk") {
        throw new Error("The application bundle has no entry chunk");
      }
      code = chunk.code;
      order = chunk.moduleIds;
    } finally {
      await bundle.close();
    }
    self.postMessage({
      kind: "built",
      id: data.id,
      code,
      css: [css, ...order.flatMap((id) => stylesheets.get(id) ?? [])].join("\n"),
      transformed: [...transformed],
      moduleCss: [...moduleCss],
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

// Module initialization failures must raise a worker error so the owner can replace this worker.
await rolldownBinding;
