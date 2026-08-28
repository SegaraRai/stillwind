import { randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import { dirname, join, relative } from "node:path";
import { build } from "esbuild";
import { compileReact, generateReactTypes } from "@stillwind/native";
import type { Plugin, ViteDevServer } from "vite";

export interface StillwindOptions {
  /** Absolute path to the CSS file containing the project's `@theme` tokens. */
  theme: string;
  /** Emits a module augmentation with the CSS theme's utility vocabulary. */
  types?: false | string;
}

interface CompilerOutput {
  css: string;
  code: string | null;
  classes: Array<{ class_name: string }>;
  symbols: GeneratedSymbol[];
  diagnostics: Array<{
    code: string;
    message: string;
    file: string;
    start: number;
    end: number;
    start_utf16: number;
    end_utf16: number;
    start_line: number;
    start_column: number;
    end_line: number;
    end_column: number;
    notes: string[];
  }>;
}

interface GeneratedSymbol {
  name: string;
  kind: string;
  identity: string;
  source_path: string;
  start: number;
  end: number;
}

interface PlayOutput extends CompilerOutput {
  previewUrl: string | null;
}

const virtualCssId = "virtual:stillwind.css";
const resolvedVirtualCssId = `\0${virtualCssId}`;
const maximumPlayPayloadBytes = 1_000_000;

function compile(source: string, path: string, theme: string): CompilerOutput {
  return JSON.parse(compileReact(source, path, theme)) as CompilerOutput;
}

function compilePlay(source: string, theme: string): CompilerOutput {
  // Play accepts the pleasant `export App;` shorthand in addition to regular
  // TypeScript exports. This replacement preserves byte length for source spans.
  return compile(source.replace(/\bexport App;/g, "export{App}"), "Play.tsx", theme);
}

function compilerModulePath(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

function findSymbolCollision(
  symbolsByModule: ReadonlyMap<string, readonly GeneratedSymbol[]>,
  moduleId: string,
  next: readonly GeneratedSymbol[],
): [GeneratedSymbol, GeneratedSymbol] | null {
  const names = new Map<string, GeneratedSymbol>();
  for (const [candidateId, symbols] of symbolsByModule) {
    if (candidateId === moduleId) continue;
    for (const symbol of symbols) names.set(symbol.name, symbol);
  }
  for (const symbol of next) {
    const existing = names.get(symbol.name);
    if (existing && existing.identity !== symbol.identity) return [existing, symbol];
    names.set(symbol.name, symbol);
  }
  return null;
}

function sendJson(response: ServerResponse, value: unknown, statusCode = 200) {
  response.statusCode = statusCode;
  response.setHeader("content-type", "application/json; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.end(JSON.stringify(value));
}

function readJson(request: IncomingMessage): Promise<{ source: string; theme: string }> {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk: string) => {
      body += chunk;
      if (Buffer.byteLength(body) > maximumPlayPayloadBytes)
        reject(new Error("playground input exceeds 1 MB"));
    });
    request.on("error", reject);
    request.on("end", () => {
      try {
        const value = JSON.parse(body) as Partial<{ source: unknown; theme: unknown }>;
        if (typeof value.source !== "string" || typeof value.theme !== "string") {
          throw new Error("source and theme must be strings");
        }
        resolve({ source: value.source, theme: value.theme });
      } catch (error) {
        reject(error);
      }
    });
  });
}

function previewSource(code: string): string {
  const withoutMacroImport = code.replace(
    /^\s*import\s+(?:[^;]+?)\s+from\s+["']stillwind\/react["'];?\s*$/gm,
    "",
  );
  const withRuntime = withoutMacroImport.includes("stillwindClsx(")
    ? 'import { stillwindClsx } from "stillwind/react/runtime";\n' + withoutMacroImport
    : withoutMacroImport;
  return withRuntime.replace(/\bexport\s+App\s*;/g, "export { App };");
}

async function bundlePreview(source: string, css: string, server: ViteDevServer): Promise<string> {
  const userModuleId = "stillwind-play-user";
  const userSource = previewSource(source);
  const appImport = /\bexport\s+default\b/.test(userSource)
    ? `import App from "${userModuleId}";`
    : /\bexport\s+(?:function|const|let|class)\s+App\b|\bexport\s*\{\s*App\s*\}/.test(userSource)
      ? `import { App } from "${userModuleId}";`
      : null;
  if (!appImport)
    throw new Error(
      "Play expects `export default App`, `export function App`, or `export { App }`.",
    );
  const entry = `
    import * as React from "react";
    import { createRoot } from "react-dom/client";
    ${appImport}
    if (typeof App !== "function") throw new Error("Play expects an App component.");
    const root = document.getElementById("root");
    if (!root) throw new Error("Preview root is missing.");
    createRoot(root).render(React.createElement(App));
  `;
  const result = await build({
    absWorkingDir: server.config.root,
    stdin: {
      contents: entry,
      resolveDir: server.config.root,
      sourcefile: "ShorthPlayEntry.tsx",
      loader: "tsx",
    },
    bundle: true,
    format: "iife",
    jsx: "automatic",
    minify: true,
    platform: "browser",
    sourcemap: "inline",
    target: ["es2022"],
    write: false,
    plugins: [
      {
        name: "stillwind-play-user-module",
        setup(esbuild) {
          esbuild.onResolve({ filter: new RegExp(`^${userModuleId}$`) }, () => ({
            path: userModuleId,
            namespace: "stillwind-play",
          }));
          esbuild.onLoad({ filter: /.*/, namespace: "stillwind-play" }, () => ({
            contents: userSource,
            loader: "tsx",
            resolveDir: server.config.root,
          }));
        },
      },
    ],
  });
  const javascript = result.outputFiles[0]?.text;
  if (!javascript) throw new Error("preview bundle did not produce JavaScript");
  return `
window.addEventListener("error", (event) => parent.postMessage({type:"stillwind-preview-error",message:event.message}, "*"));
window.addEventListener("unhandledrejection", (event) => parent.postMessage({type:"stillwind-preview-error",message:String(event.reason)}, "*"));
const previewStyle = document.createElement("style");
previewStyle.textContent = ${JSON.stringify(css)};
document.head.append(previewStyle);
try { ${javascript} } catch (error) { parent.postMessage({type:"stillwind-preview-error",message:error instanceof Error ? error.message : String(error)}, "*"); }
`;
}

function previewHtml(id: string) {
  const encodedId = encodeURIComponent(id);
  return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors 'self'"><style>html,body,#root{min-height:100%;margin:0}</style></head><body><div id="root"></div><script src="/__stillwind/preview-script?id=${encodedId}"></script></body></html>`;
}

/** Runs the native compiler and renders Play user modules only inside a sandboxed iframe. */
export function stillwind(options: StillwindOptions): Plugin {
  const cssByModule = new Map<string, string>();
  const symbolsByModule = new Map<string, GeneratedSymbol[]>();
  const previewBundles = new Map<string, string>();
  let projectRoot = "";

  return {
    name: "stillwind",
    enforce: "pre",
    resolveId(id) {
      return id === virtualCssId ? resolvedVirtualCssId : undefined;
    },
    load(id) {
      if (id === resolvedVirtualCssId) return [...cssByModule.values()].join("\n");
      return undefined;
    },
    configResolved(config) {
      projectRoot = config.root;
      if (options.types === false) return;
      const output = options.types ?? join(config.root, "src", "stillwind.generated.d.ts");
      const path =
        output.startsWith("/") || /^[A-Za-z]:[\\/]/.test(output)
          ? output
          : join(config.root, output);
      const types = generateReactTypes(readFileSync(options.theme, "utf8"));
      try {
        if (readFileSync(path, "utf8") === types) return;
      } catch {
        // The generated file is intentionally created only when it changes.
      }
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, types);
    },
    transform(code, id) {
      if (!/\.[cm]?[jt]sx?$/.test(id) || id.includes("node_modules")) return undefined;
      if (!code.includes("stillwind/react")) {
        cssByModule.delete(id);
        symbolsByModule.delete(id);
        return undefined;
      }
      const output = compile(
        code,
        compilerModulePath(projectRoot, id),
        readFileSync(options.theme, "utf8"),
      );
      if (output.diagnostics.length > 0) {
        const diagnostic = output.diagnostics[0];
        this.error(`${diagnostic.code}: ${diagnostic.message} (${id}:${diagnostic.start})`);
      }
      if (!output.code) return undefined;
      const collision = findSymbolCollision(symbolsByModule, id, output.symbols);
      if (collision) {
        const [first, second] = collision;
        this.error(
          `Stillwind generated identifier collision: ${second.name}\n` +
            `  ${first.kind} at ${first.source_path}:${first.start}\n` +
            `  ${second.kind} at ${second.source_path}:${second.start}`,
        );
      }
      symbolsByModule.set(id, output.symbols);
      cssByModule.set(id, output.css);
      const runtimeImport = output.code.includes("stillwindClsx(")
        ? 'import { stillwindClsx } from "stillwind/react/runtime";\n'
        : "";
      return { code: `${runtimeImport}import "${virtualCssId}";\n${output.code}`, map: null };
    },
    configureServer(server: ViteDevServer) {
      server.middlewares.use("/__stillwind/compile", (request, response) => {
        if (request.method !== "POST")
          return sendJson(response, { error: "POST is required" }, 405);
        void readJson(request)
          .then(async ({ source, theme }) => {
            const output = compilePlay(source, theme);
            let previewUrl: string | null = null;
            if (output.diagnostics.length === 0 && output.code) {
              try {
                const id = randomUUID();
                previewBundles.set(id, await bundlePreview(output.code, output.css, server));
                while (previewBundles.size > 12)
                  previewBundles.delete(previewBundles.keys().next().value!);
                previewUrl = `/__stillwind/preview?id=${encodeURIComponent(id)}`;
              } catch (error) {
                output.diagnostics.push({
                  code: "SHPLAY001",
                  message: error instanceof Error ? error.message : String(error),
                  file: "Play.tsx",
                  start: 0,
                  end: 0,
                  start_utf16: 0,
                  end_utf16: 0,
                  start_line: 0,
                  start_column: 0,
                  end_line: 0,
                  end_column: 0,
                  notes: [],
                });
              }
            }
            sendJson(response, { ...output, previewUrl } satisfies PlayOutput);
          })
          .catch((error: unknown) =>
            sendJson(
              response,
              { error: error instanceof Error ? error.message : String(error) },
              400,
            ),
          );
      });
      server.middlewares.use("/__stillwind/preview", (request, response) => {
        const id = new URL(request.url ?? "", "http://stillwind.local").searchParams.get("id");
        if (!id || !previewBundles.has(id)) {
          response.statusCode = 404;
          return response.end("Preview expired");
        }
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.setHeader("cache-control", "no-store");
        response.end(previewHtml(id));
      });
      server.middlewares.use("/__stillwind/preview-script", (request, response) => {
        const id = new URL(request.url ?? "", "http://stillwind.local").searchParams.get("id");
        const bundle = id ? previewBundles.get(id) : undefined;
        if (!bundle) {
          response.statusCode = 404;
          return response.end("Preview expired");
        }
        response.setHeader("content-type", "text/javascript; charset=utf-8");
        response.setHeader("cache-control", "no-store");
        response.end(bundle);
      });
    },
  };
}
