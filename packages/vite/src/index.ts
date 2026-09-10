import { randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { dirname, relative, resolve } from "node:path";

import { NativeDesignSystemLoader, NativeProjectSession, summarizeReact } from "@stillwind/native";
import { build } from "esbuild";
import type { Plugin, ViteDevServer } from "vite-plus";

import { moduleSourceMap, type CompilerMapping } from "./source-map.ts";
import { TypeScriptFactsService, type AssetRecord, type TypeFact } from "./type-facts.ts";

export interface StillwindOptions {
  /** Absolute path to the CSS file containing the project's `@theme` tokens. */
  theme?: string;
  /** CSS-first entrypoint. `theme` is retained as a compatibility alias. */
  css?: string;
  /** Emits a module augmentation with the CSS theme's utility vocabulary. */
  types?: false | string;
  /** Namespace for Stillwind-owned CSS identifiers. */
  prefix?: string;
  /** Namespace for Tailwind-compatible internal custom properties. */
  tailwindPrefix?: string;
  /** Reject Tailwind group/peer/container relationships in favor of typed handles. */
  forbidTailwindRelationships?: boolean;
  /** Module identity isolates generated classes. Content identity permits sharing. */
  classIdentity?: "module" | "content";
  /** Stable namespace shared by the project's client and server builds. */
  classNamespace?: string;
}

interface CompilerOutput {
  mappings: CompilerMapping[];
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
  lint_findings: Array<{
    rule: string;
    message: string;
    file: string;
    start: number;
    end: number;
    notes: string[];
  }>;
  lint_policy: Record<string, "warn" | "deny" | "off">;
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

interface ModuleSummary {
  type_probes: Array<{ start: number; end: number }>;
  imports: Array<{ source: string }>;
  re_exports: Array<{ source: string }>;
  assets: AssetRecord[];
}

interface ProjectSource {
  id: string;
  path: string;
  source: string;
  type_facts?: TypeFact[];
}

interface ProjectCompilerOutput {
  modules: Array<CompilerOutput & { path: string }>;
}

const virtualCssId = "virtual:stillwind.css";
const resolvedVirtualCssId = `\0${virtualCssId}`;
const moduleCssPrefix = "virtual:stillwind/module.css?css=";
const maximumPlayPayloadBytes = 1_000_000;

function cssEntrypoint(options: StillwindOptions): string {
  const path = options.css ?? options.theme;
  if (!path) throw new Error("Stillwind requires a CSS-first entrypoint (`css`)");
  return path;
}

function compilerOptions(options: StillwindOptions): string {
  return JSON.stringify({
    prefix: options.prefix ?? "--sw-",
    tailwindPrefix: options.tailwindPrefix ?? "--tw-",
    forbidTailwindRelationships: options.forbidTailwindRelationships ?? false,
    classIdentity: options.classIdentity ?? "module",
    classNamespace: options.classNamespace ?? "",
  });
}

function compilerModulePath(root: string, id: string): string {
  return relative(root, id).replaceAll("\\", "/");
}

function cleanModuleId(id: string): string {
  return id.replace(/[?#].*$/, "");
}

function localDependencies(summary: ModuleSummary): string[] {
  return [
    ...summary.imports.map((entry) => entry.source),
    ...summary.re_exports.map((entry) => entry.source),
  ].filter(
    (specifier, index, all) => specifier.startsWith(".") && all.indexOf(specifier) === index,
  );
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
  const withRuntime = finalizeReactModule(code, false);
  return withRuntime.replace(/\bexport\s+App\s*;/g, "export { App };");
}

/** Adds the runtime helpers (and optionally virtual CSS) required by compiled output. */
export function finalizeReactModule(code: string, includeStylesheet = true): string {
  return finalizeReactModuleWithMap(code, includeStylesheet).code;
}

export type FinalizedModuleEdit = { from: number; to: number };

/** Finalizes compiled output while retaining edits needed to remap source spans. */
export function finalizeReactModuleWithMap(code: string, includeStylesheet = true) {
  const macroImportPattern = /^\s*import\s+(?:[^;]+?)\s+from\s+["']stillwind\/react["'];?\s*$/gm;
  const removed = [...code.matchAll(macroImportPattern)].map((match) => ({
    from: match.index,
    to: match.index + match[0].length,
  }));
  const withoutMacroImport = code.replace(macroImportPattern, "");
  const runtimeImport = renderRuntimeImport(withoutMacroImport);
  const stylesheetImport = includeStylesheet ? `import "${virtualCssId}";\n` : "";
  const prefix = runtimeImport + stylesheetImport;
  return { code: prefix + withoutMacroImport, prefix, removed };
}

function renderRuntimeImport(code: string): string {
  const names = [
    "stillwindAnimation",
    "stillwindAssetClass",
    "stillwindClsx",
    "stillwindHandleClass",
    "stillwindRef",
    "stillwindScope",
    "stillwindViewTransition",
    "stillwindViewTransitionStyle",
  ].filter((name) => code.includes(`${name}(`));
  return names.length > 0 ? `import { ${names.join(", ")} } from "stillwind/react/runtime";\n` : "";
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
      sourcefile: "StillwindPlayEntry.tsx",
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
  type EnvironmentState = {
    session?: NativeProjectSession;
    resources: Map<string, string>;
    producers: Set<string>;
    cssFiles: Set<string>;
    typeFacts?: TypeScriptFactsService;
    revision: number;
  };
  const environments = new Map<string, EnvironmentState>();
  const environmentState = (name = "client") => {
    let state = environments.get(name);
    if (!state) {
      state = { resources: new Map(), producers: new Set(), cssFiles: new Set(), revision: 0 };
      environments.set(name, state);
    }
    return state;
  };
  let watching = false;
  const cssFiles = new Set<string>();
  const previewBundles = new Map<string, string>();
  let projectRoot = "";
  const loaderResources = new WeakMap<NativeDesignSystemLoader, Map<string, string>>();
  const resolveResources = (css: string, references = new Map<string, string>()) => {
    for (const [placeholder, url] of references) css = css.replaceAll(placeholder, url);
    return css;
  };

  const closeTypeFacts = () => {
    for (const state of environments.values()) {
      state.typeFacts?.close();
      state.typeFacts = undefined;
    }
  };

  const loadDesignSystem = async (
    resolveImport: (specifier: string, importer: string) => Promise<string>,
    rootContent?: string,
    dependencies = cssFiles,
  ) => {
    const loader = new NativeDesignSystemLoader(compilerOptions(options));
    const root = realpathSync(resolve(projectRoot, cssEntrypoint(options)));
    dependencies.add(root);
    type LoadStep =
      | { kind: "needFile"; requests: { id: number; importer: string; specifier: string }[] }
      | {
          kind: "complete";
          resources: { placeholder: string; owner: string; specifier: string }[];
        };
    let resourceReferences: Extract<LoadStep, { kind: "complete" }>["resources"] = [];
    const initial = JSON.parse(
      loader.provide(0, root, rootContent ?? (await readFile(root, "utf8"))),
    ) as LoadStep;
    await new Promise<void>((resolveLoad, rejectLoad) => {
      const started = new Set<number>();
      let stopped = false;
      const fail = (error: unknown) => {
        stopped = true;
        rejectLoad(error);
      };
      const advance = (step: LoadStep) => {
        if (step.kind === "complete") {
          resourceReferences = step.resources;
          stopped = true;
          resolveLoad();
          return;
        }
        for (const request of step.requests) {
          // NeedFile repeats outstanding IDs, including reads already in flight.
          if (started.has(request.id)) continue;
          started.add(request.id);
          const read = async () => {
            const name = await realpath(await resolveImport(request.specifier, request.importer));
            if (stopped) return;
            dependencies.add(name);
            const content = await readFile(name, "utf8");
            if (stopped) return;
            // JS callbacks serialize provide/step processing; I/O remains concurrent.
            advance(JSON.parse(loader.provide(request.id, name, content)) as LoadStep);
          };
          // Handle every rejection, including reads finishing after another read failed.
          void read().catch(fail);
        }
      };
      advance(initial);
    });
    const references = new Map<string, string>();
    for (const resource of resourceReferences) {
      const resolved = await resolveImport(resource.specifier, resource.owner);
      const url = `/@fs/${resolved.replaceAll("\\", "/")}`;
      references.set(resource.placeholder, JSON.stringify(url).slice(1, -1));
      dependencies.add(cleanModuleId(resolved));
    }
    loaderResources.set(loader, references);
    return loader;
  };

  const writeTypes = (loader: NativeDesignSystemLoader) => {
    if (options.types === false) return;
    const path = resolve(projectRoot, options.types ?? "src/stillwind.generated.d.ts");
    const types = loader.generateReactTypes();
    try {
      if (readFileSync(path, "utf8") === types) return;
    } catch {
      /* First generation. */
    }
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, types);
  };

  return {
    name: "stillwind",
    enforce: "pre",
    perEnvironmentStartEndDuringDev: true,
    perEnvironmentWatchChangeDuringDev: true,
    resolveId(id) {
      return id === virtualCssId || id.startsWith(moduleCssPrefix) ? `\0${id}` : undefined;
    },
    load(id) {
      const state = environmentState(this.environment?.name);
      if (id === resolvedVirtualCssId)
        return resolveResources(state.session!.baseCss(), state.resources);
      if (id.startsWith(`\0${moduleCssPrefix}`))
        return Buffer.from(id.slice(moduleCssPrefix.length + 1), "base64url").toString("utf8");
      return undefined;
    },
    configResolved(config) {
      projectRoot = config.root;
      watching = config.command === "serve";
      closeTypeFacts();
    },
    async buildStart() {
      const state = environmentState(this.environment?.name);
      const revision = ++state.revision;
      const dependencies = new Set<string>();
      const loader = await loadDesignSystem(
        async (specifier, importer) => {
          const result = await this.resolve(specifier, importer, { skipSelf: true });
          if (!result || result.external)
            throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
          return result.id;
        },
        undefined,
        dependencies,
      );
      if (state.revision !== revision) return;
      state.session = new NativeProjectSession(loader, watching);
      state.resources = loaderResources.get(loader) ?? new Map();
      state.cssFiles = dependencies;
      state.producers.clear();
      for (const path of dependencies) this.addWatchFile(path);
      writeTypes(loader);
    },
    async hotUpdate(context) {
      const state = environmentState(this.environment.name);
      if (!state.cssFiles.has(context.file) && !state.cssFiles.has(resolve(context.file))) return;
      const revision = ++state.revision;
      const dependencies = new Set<string>();
      try {
        const loader = await loadDesignSystem(
          async (specifier, importer) => {
            const result = await this.environment.pluginContainer.resolveId(specifier, importer);
            if (!result || result.external)
              throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
            return result.id;
          },
          undefined,
          dependencies,
        );
        if (revision !== state.revision) return [];
        const update = JSON.parse(state.session!.reload(loader)) as {
          cssChanged: boolean;
          transformedModules: string[];
        };
        state.resources = loaderResources.get(loader) ?? new Map();
        state.cssFiles = dependencies;
        context.server.watcher.add([...dependencies]);
        writeTypes(loader);
        // A CSS-only semantic change must also refresh the producer's payload
        // import, even when its class names and JS transforms are unchanged.
        const modules = [...state.producers].flatMap((id) => [
          ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(id)) ?? []),
        ]);
        const cssModule = this.environment.moduleGraph.getModuleById(resolvedVirtualCssId);
        if (update.cssChanged && cssModule) modules.push(cssModule);
        for (const module of modules) this.environment.moduleGraph.invalidateModule(module);
        return modules;
      } catch (error) {
        this.warn(
          `Stillwind CSS reload failed; retaining last successful output: ${String(error)}`,
        );
        return [];
      }
    },
    watchChange(id, change) {
      const state = environmentState(this.environment?.name);
      if (change.event === "delete") {
        state.session?.remove(compilerModulePath(projectRoot, id));
        state.producers.delete(id);
      }
    },
    async transform(code, id) {
      if (!/\.[cm]?[jt]sx?$/.test(id) || id.includes("node_modules")) return undefined;
      const state = environmentState(this.environment?.name);
      const session = state.session!;
      if (!code.includes("stillwind/react")) {
        session.remove(compilerModulePath(projectRoot, cleanModuleId(id)));
        state.producers.delete(id);
        return undefined;
      }
      const entryId = cleanModuleId(id);
      const entrySummary = JSON.parse(
        summarizeReact(code, compilerModulePath(projectRoot, entryId)),
      ) as ModuleSummary;
      const needsTypeFacts = entrySummary.type_probes.length > 0;
      const summaries = new Map<string, ModuleSummary>();
      const sourcesById = new Map<string, ProjectSource>();
      const pending: ProjectSource[] = [
        { id: entryId, path: compilerModulePath(projectRoot, entryId), source: code },
      ];
      while (pending.length > 0) {
        const source = pending.pop()!;
        if (sourcesById.has(source.id)) continue;
        sourcesById.set(source.id, source);
        const summary =
          source.id === entryId
            ? entrySummary
            : (JSON.parse(summarizeReact(source.source, source.path)) as ModuleSummary);
        summaries.set(source.path, summary);
        if (!needsTypeFacts) continue;
        for (const specifier of localDependencies(summary)) {
          const resolved = await this.resolve(specifier, source.id, { skipSelf: true });
          if (!resolved || resolved.external) continue;
          const dependencyId = cleanModuleId(resolved.id);
          if (!/\.[cm]?[jt]sx?$/.test(dependencyId) || sourcesById.has(dependencyId)) continue;
          this.addWatchFile(dependencyId);
          pending.push({
            id: dependencyId,
            path: compilerModulePath(projectRoot, dependencyId),
            source: readFileSync(dependencyId, "utf8"),
          });
        }
      }

      const sources = [...sourcesById.values()];
      const typeFacts = needsTypeFacts
        ? (state.typeFacts ??= new TypeScriptFactsService(projectRoot)).collect(
            sources,
            new Map([...summaries].map(([path, summary]) => [path, summary.assets])),
          )
        : new Map<string, TypeFact[]>();
      const typedSources = sources.map((source) => ({
        ...source,
        type_facts: typeFacts.get(source.path) ?? [],
      }));
      const projectOutput = JSON.parse(
        session.compile(JSON.stringify(typedSources), compilerModulePath(projectRoot, entryId)),
      ) as ProjectCompilerOutput;
      const idsByPath = new Map(sources.map((source) => [source.path, source.id]));
      for (const output of projectOutput.modules) {
        const moduleId = idsByPath.get(output.path);
        if (!moduleId) this.error(`Stillwind returned an unknown module: ${output.path}`);
        if (output.diagnostics.length > 0) {
          const diagnostic = output.diagnostics[0];
          this.error(
            `${diagnostic.code}: ${diagnostic.message} (${diagnostic.file}:${diagnostic.start})`,
          );
        }
        const deniedFindings: string[] = [];
        for (const finding of output.lint_findings ?? []) {
          const level = output.lint_policy?.[finding.rule] ?? "warn";
          if (level !== "deny") {
            this.warn(`${finding.rule}: ${finding.message} (${finding.file}:${finding.start})`);
          } else {
            deniedFindings.push(
              `${finding.rule}: ${finding.message} (${finding.file}:${finding.start})`,
            );
          }
        }
        if (deniedFindings.length > 0) this.error(deniedFindings.join("\n"));
      }
      const output = projectOutput.modules.find(
        (candidate) => candidate.path === compilerModulePath(projectRoot, entryId),
      );
      if (!output?.code) return undefined;
      state.producers.add(id);
      const css = resolveResources(session.moduleCss(output.path), state.resources);
      const carrier = `${moduleCssPrefix}${Buffer.from(css).toString("base64url")}`;
      const finalized = finalizeReactModuleWithMap(output.code, false);
      const imports = `import "${virtualCssId}";\nimport ${JSON.stringify(carrier)};\n`;
      return {
        code: imports + finalized.code,
        map: JSON.stringify(
          moduleSourceMap(
            code,
            output.code,
            id,
            output.mappings,
            finalized.removed,
            imports + finalized.prefix,
          ),
        ),
      };
    },
    configureServer(server: ViteDevServer) {
      server.httpServer?.once("close", closeTypeFacts);
      server.middlewares.use("/__stillwind/compile", (request, response) => {
        if (request.method !== "POST")
          return sendJson(response, { error: "POST is required" }, 405);
        void readJson(request)
          .then(async ({ source, theme }) => {
            const playLoader = await loadDesignSystem(
              async (specifier, importer) => {
                const result = await server.pluginContainer.resolveId(specifier, importer);
                if (!result || result.external)
                  throw new Error(`Cannot resolve CSS ${specifier} from ${importer}`);
                return result.id;
              },
              theme,
              new Set(),
            );
            const playSession = new NativeProjectSession(playLoader, false);
            // Preserve byte length for the Play shorthand's source positions.
            const playProject = JSON.parse(
              playSession.compile(
                JSON.stringify([
                  { path: "Play.tsx", source: source.replace(/\bexport App;/g, "export{App}") },
                ]),
              ),
            ) as ProjectCompilerOutput;
            const output = playProject.modules[0]!;
            output.css = resolveResources(playSession.css(), loaderResources.get(playLoader));
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
    closeBundle: closeTypeFacts,
  };
}
