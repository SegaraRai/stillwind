import { randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { dirname, isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  compileModule,
  cssWithSourceMap,
  decodeCssSourceMap,
  loadCssGraph,
  compilerModulePath,
  cleanModuleId,
  moduleSourceMap,
  TypeScriptFactsService,
  finalizeReactModuleWithMap as finalizeModuleWithMap,
} from "@stillwind/host";
import type { CssLocations, CssUpstreamMap, PlayOutput } from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import type { NativeIdentifierNaming } from "@stillwind/native";
import { build } from "esbuild";
import type { Plugin, ViteDevServer } from "vite-plus";

/**
 * Project configuration for the Stillwind Vite plugin. `css` or `theme` must identify
 * an entry stylesheet; `css` takes precedence when both are supplied. Relative paths
 * are resolved against Vite's project root.
 */
export interface StillwindOptions {
  /** Entry stylesheet alias used when `css` is absent; absolute or project-root relative. */
  theme?: string;
  /** Entry stylesheet defining the design system; takes precedence over `theme`. */
  css?: string;
  /**
   * Destination for the theme vocabulary's module augmentation, absolute or project-root
   * relative. Defaults to `src/stillwind.generated.d.ts`; `false` disables generation.
   */
  types?: false | string;
  /** Namespace for Stillwind-owned CSS identifiers. */
  prefix?: string;
  /** Namespace for Tailwind-compatible internal custom properties. */
  tailwindPrefix?: string;
  /** Reject Tailwind group/peer/container relationships in favor of typed handles. */
  forbidTailwindRelationships?: boolean;
  /**
   * Stable namespace included in generated class identities. Use the same value for
   * client and server builds; changing it changes those identities. Defaults to empty.
   */
  classNamespace?: string;
  /**
   * Emit native CSS if() for experimental values and variants; false uses fallback helpers.
   * Defaults to false. Comparisons, negated variants, and CSS-wide helper branches require true.
   */
  useCssIf?: boolean;
  /**
   * Host name generators, one optional function per identifier family. A function
   * returns the name to write, or `undefined` to keep the compiler's own name. A
   * generator must be pure: the same request has to produce the same name. Keep the
   * functions stable for the life of the session, and update `namingRevision` with them.
   */
  naming?: NativeIdentifierNaming;
  /** Hash of `naming`, used to invalidate cached artifacts when it changes. */
  namingRevision?: string;
}

const virtualCssId = "virtual:stillwind.css";
const resolvedVirtualCssId = fileURLToPath(
  new URL("../stillwind-base.css", import.meta.url),
).replaceAll("\\", "/");
const moduleCssPrefix = "virtual:stillwind/module.css?css=";
const registeredCssPrefix = "virtual:stillwind/module.css?ref=";
const moduleCssValuePrefix = "virtual:stillwind/module-css?ref=";
const resolvedModuleCssValueId = fileURLToPath(
  new URL("../stillwind-module-css.js", import.meta.url),
).replaceAll("\\", "/");
const resolvedModuleCssId = fileURLToPath(
  new URL("../stillwind-carrier.css", import.meta.url),
).replaceAll("\\", "/");
const resolvedProjectionId = fileURLToPath(
  new URL("../stillwind-projection.js", import.meta.url),
).replaceAll("\\", "/");
const maximumPlayPayloadBytes = 1_000_000;

function resolvedFile(
  result: { id: string; external?: boolean | string } | null,
  description: string,
): string {
  if (!result) throw new Error(`Cannot resolve ${description}`);
  if (result.id.startsWith("file:")) return fileURLToPath(result.id);
  if (!result.external || isAbsolute(result.id)) return result.id;
  throw new Error(`Cannot read external ${description}: ${result.id}`);
}

function cssEntrypoint(options: StillwindOptions): string {
  const path = options.css ?? options.theme;
  if (!path) throw new Error("Stillwind requires a CSS-first entrypoint (`css`)");
  return path;
}

function compilerOptions(options: StillwindOptions) {
  return {
    prefix: options.prefix ?? "--sw-",
    tailwindPrefix: options.tailwindPrefix ?? "--tw-",
    forbidTailwindRelationships: options.forbidTailwindRelationships ?? false,
    classNamespace: options.classNamespace ?? "",
    useCssIf: options.useCssIf ?? false,
    namingRevision: options.namingRevision ?? "",
  };
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

export function finalizeReactModule(code: string, includeStylesheet = true) {
  return finalizeReactModuleWithMap(code, includeStylesheet).code;
}
export function finalizeReactModuleWithMap(code: string, includeStylesheet = true) {
  return finalizeModuleWithMap(code, includeStylesheet ? virtualCssId : undefined);
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
    initializing?: Promise<void>;
    projectionDependencies: Map<string, Set<string>>;
    resources: Map<string, string>;
    producers: Set<string>;
    compositionDependencies: Map<string, Set<string>>;
    compositionTopologies: Map<string, string>;
    cssFiles: Set<string>;
    typeFacts?: TypeScriptFactsService;
    revision: number;
  };
  const environments = new Map<string, EnvironmentState>();
  const carrierPayloads = new Map<string, string>();
  let carrierSequence = 0;
  const environmentState = (name = "client") => {
    let state = environments.get(name);
    if (!state) {
      state = {
        resources: new Map(),
        producers: new Set(),
        compositionDependencies: new Map(),
        compositionTopologies: new Map(),
        cssFiles: new Set(),
        projectionDependencies: new Map(),
        revision: 0,
      };
      environments.set(name, state);
    }
    return state;
  };
  let watching = false;
  // The compiler decides name validation from this mode; a dev server keeps compiling
  // when a host generator is rejected, a build fails instead.
  let compilerMode = "release";
  const cssFiles = new Set<string>();
  const previewBundles = new Map<string, string>();
  let projectRoot = "";
  const loaderResources = new WeakMap<NativeDesignSystemLoader, Map<string, string>>();
  const resolveResources = (css: string, references = new Map<string, string>()) => {
    for (const [placeholder, url] of references) css = css.replaceAll(placeholder, url);
    return css;
  };
  const carrierSpecifier = (_state: EnvironmentState, css: string) => {
    // Development requests must not put a complete source map in the HTTP URL.
    // A fresh reference also prevents delayed HMR pruning from removing restored CSS.
    if (watching) {
      const reference = String(++carrierSequence);
      carrierPayloads.set(reference, css);
      return `${registeredCssPrefix}${reference}&lang.css`;
    }
    // Production transport remains self-contained across independent cached transforms.
    return `${moduleCssPrefix}${Buffer.from(css).toString("base64url")}&lang.css`;
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
    const root = realpathSync(resolve(projectRoot, cssEntrypoint(options)));
    const { loader, resources: resourceReferences } = await loadCssGraph(
      root,
      compilerOptions(options),
      {
        resolve: async (specifier, importer) => realpath(await resolveImport(specifier, importer)),
        read: (name) => readFile(name, "utf8"),
        watch: (name) => dependencies.add(name),
      },
      rootContent,
    );
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

  const ensureSession = async (
    state: EnvironmentState,
    context: {
      resolve: (
        specifier: string,
        importer: string,
        options: { skipSelf: boolean },
      ) => Promise<{ id: string; external?: boolean | string } | null>;
      addWatchFile: (id: string) => void;
    },
  ) => {
    if (state.session) return;
    if (!state.initializing) {
      const revision = ++state.revision;
      const initialize = async () => {
        const dependencies = new Set<string>();
        const loader = await loadDesignSystem(
          async (specifier, importer) => {
            const result = await context.resolve(specifier, importer, { skipSelf: true });
            return resolvedFile(result, `CSS ${specifier} from ${importer}`);
          },
          undefined,
          dependencies,
        );
        if (state.revision !== revision) return;
        state.session = new NativeProjectSession(loader, watching, compilerMode, options.naming);
        state.resources = loaderResources.get(loader) ?? new Map();
        state.cssFiles = dependencies;
        state.producers.clear();
        for (const path of dependencies) context.addWatchFile(path);
        writeTypes(loader);
      };
      state.initializing = initialize().finally(() => {
        state.initializing = undefined;
      });
    }
    await state.initializing;
  };

  const renderCarrier = async (
    state: EnvironmentState,
    modulePath: string,
    host: {
      resolve(
        specifier: string,
        importer: string,
      ): Promise<{ id: string; external?: boolean | string } | null>;
      watch(id: string): void;
      upstream?: CssUpstreamMap;
      includeSourcesContent?: boolean;
      placement?: "document" | "carrier";
    },
  ) => {
    const plan = state.session!.planStylesheet({
      modules: [modulePath],
      coverage: { kind: "observed" },
      placement: host.placement ?? "carrier",
    }) as {
      css: string;
      sourceMap: CssLocations;
      coverage: { kind: string };
      designSystem: string;
      producers: unknown[];
      resources: { placeholder: string; owner: string; specifier: string }[];
    };
    const resources = new Map(state.resources);
    for (const resource of plan.resources) {
      const resolved = await host.resolve(resource.specifier, resolve(projectRoot, resource.owner));
      const file = resolvedFile(
        resolved,
        `Stillwind resource ${resource.specifier} from ${resource.owner}`,
      );
      host.watch(cleanModuleId(file));
      resources.set(
        resource.placeholder,
        JSON.stringify(`/@fs/${file.replaceAll("\\", "/")}`).slice(1, -1),
      );
    }
    return {
      ...plan,
      css: cssWithSourceMap(
        plan,
        resources,
        projectRoot,
        host.upstream,
        host.includeSourcesContent,
      ),
    };
  };

  return {
    name: "stillwind",
    enforce: "pre",
    perEnvironmentStartEndDuringDev: true,
    perEnvironmentWatchChangeDuringDev: true,
    async resolveId(id, importer) {
      if (id.startsWith(resolvedModuleCssId)) {
        // Some framework asset collectors append ?inline to an existing query.
        return id.replace(/\?inline$/, "&inline");
      }
      if (importer && cleanModuleId(importer) === resolvedProjectionId) {
        const state = environmentState(this.environment?.name);
        if (state.projectionDependencies.get(id.replaceAll("\\", "/"))?.has(importer)) {
          return `${id}?stillwind-css-watch`;
        }
      }
      if (id.endsWith("?stillwind-css-watch")) return id;
      if (id === virtualCssId) return resolvedVirtualCssId;
      if (id.startsWith(moduleCssPrefix))
        return `${resolvedModuleCssId}?css=${id.slice(moduleCssPrefix.length)}`;
      if (id.startsWith(registeredCssPrefix))
        return `${resolvedModuleCssId}?ref=${id.slice(registeredCssPrefix.length)}`;
      if (id.startsWith(moduleCssValuePrefix))
        return `${resolvedModuleCssValueId}?ref=${id.slice(moduleCssValuePrefix.length)}`;
      if (id.endsWith("?stillwind.css")) {
        const source = await this.resolve(id, importer, { skipSelf: true });
        if (!source || source.external) this.error(`Cannot resolve Stillwind CSS projection ${id}`);
        return `${resolvedProjectionId}?source=${Buffer.from(cleanModuleId(source.id)).toString("base64url")}`;
      }
      return undefined;
    },
    async load(id) {
      if (id.endsWith("?stillwind-css-watch")) return "";
      const state = environmentState(this.environment?.name);
      if (cleanModuleId(id) === resolvedModuleCssValueId) {
        const reference = new URLSearchParams(id.slice(id.indexOf("?") + 1)).get("ref");
        if (reference === null || !carrierPayloads.has(reference))
          this.error("Unknown Stillwind module CSS value");
        return `export { default } from ${JSON.stringify(`${registeredCssPrefix}${reference}&inline&lang.css`)};`;
      }
      if (cleanModuleId(id) === resolvedProjectionId) {
        for (const [dependency, projections] of state.projectionDependencies) {
          projections.delete(id);
          if (projections.size === 0) state.projectionDependencies.delete(dependency);
        }
        const encoded = new URLSearchParams(id.slice(id.indexOf("?") + 1)).get("source");
        if (encoded === null) this.error("Stillwind CSS projection is missing its source");
        const sourceId = Buffer.from(encoded, "base64url").toString("utf8");
        const watch = (dependency: string) => {
          this.addWatchFile(dependency);
          const key = dependency.replaceAll("\\", "/");
          const projections = state.projectionDependencies.get(key) ?? new Set<string>();
          projections.add(id);
          state.projectionDependencies.set(key, projections);
        };
        watch(sourceId);
        await ensureSession(state, this);
        const output = await compileModule(
          state.session!,
          () => (state.typeFacts ??= new TypeScriptFactsService(projectRoot)),
          await readFile(sourceId, "utf8"),
          sourceId,
          projectRoot,
          {
            resolve: (specifier, importer) => this.resolve(specifier, importer, { skipSelf: true }),
            read: (dependency) => readFile(dependency, "utf8"),
            watch,
            warn: (message) => this.warn(message),
          },
        );
        if (!output) return "";
        // TODO: Transport base module CSS through imports from standalone projections
        // and verify dependency order when both base and derived CSS are requested.
        if (output.compositionModules.length > 0)
          this.error(
            "Cross-module style composition is not supported by standalone CSS projections; import the producer JavaScript through Vite instead.",
          );
        const css = await renderCarrier(state, output.path, {
          includeSourcesContent: false,
          resolve: (specifier, importer) => this.resolve(specifier, importer, { skipSelf: true }),
          watch,
        });
        state.producers.add(id);
        return `import ${JSON.stringify(virtualCssId)};\nimport ${JSON.stringify(carrierSpecifier(state, css.css))};`;
      }
      if (cleanModuleId(id) === resolvedVirtualCssId) {
        await ensureSession(state, this);
        return decodeCssSourceMap(
          cssWithSourceMap(state.session!.baseCssWithMap(), state.resources, projectRoot),
        );
      }
      if (cleanModuleId(id) === resolvedModuleCssId) {
        const parameters = new URLSearchParams(id.slice(id.indexOf("?") + 1));
        const reference = parameters.get("ref");
        if (reference !== null) {
          const css = carrierPayloads.get(reference);
          if (css === undefined) this.error(`Unknown Stillwind CSS carrier ${reference}`);
          return decodeCssSourceMap(css);
        }
        const payload = parameters.get("css");
        if (payload === null) this.error("Stillwind CSS carrier is missing its payload");
        return decodeCssSourceMap(Buffer.from(payload, "base64url").toString("utf8"));
      }
      return undefined;
    },
    configResolved(config) {
      projectRoot = config.root;
      watching = config.command === "serve";
      compilerMode = watching ? "development" : "release";
      closeTypeFacts();
    },
    async buildStart() {
      const state = environmentState(this.environment?.name);
      state.session = undefined;
      await ensureSession(state, this);
    },
    async hotUpdate(context) {
      const state = environmentState(this.environment.name);
      const changed = cleanModuleId(context.file).replaceAll("\\", "/");
      const hasComposition = state.compositionDependencies.size > 0;
      const touchesComposition = [...state.compositionDependencies].some(
        ([producer, dependencies]) => producer === changed || dependencies.has(changed),
      );
      // Replacing a carrier can move its style element behind a derived carrier.
      // Reload until a transport can prove it preserves that relative position.
      if (touchesComposition && !state.cssFiles.has(context.file)) {
        for (const module of this.environment.moduleGraph.idToModuleMap.values()) {
          this.environment.moduleGraph.invalidateModule(module);
        }
        this.environment.hot.send({ type: "full-reload" });
        return [];
      }
      if (!state.cssFiles.has(context.file) && !state.cssFiles.has(resolve(context.file))) {
        const projections = state.projectionDependencies.get(context.file.replaceAll("\\", "/"));
        if (!projections) return;
        const modules = [...context.modules];
        for (const id of projections) {
          const module = this.environment.moduleGraph.getModuleById(id);
          if (module) {
            this.environment.moduleGraph.invalidateModule(module);
            modules.push(module);
          }
        }
        return modules;
      }
      const revision = ++state.revision;
      const dependencies = new Set<string>();
      try {
        const loader = await loadDesignSystem(
          async (specifier, importer) => {
            const result = await this.environment.pluginContainer.resolveId(specifier, importer);
            return resolvedFile(result, `CSS ${specifier} from ${importer}`);
          },
          undefined,
          dependencies,
        );
        if (revision !== state.revision) return [];
        const update = state.session!.reload(loader) as {
          cssChanged: boolean;
          transformedModules: string[];
        };
        state.resources = loaderResources.get(loader) ?? new Map();
        state.cssFiles = dependencies;
        context.server.watcher.add([...dependencies]);
        writeTypes(loader);
        // A CSS-only semantic change must also refresh the producer's payload
        // import, even when its class names and JS transforms are unchanged.
        const modules = [
          ...new Set(
            [...state.producers].flatMap((id) => [
              ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(id)) ?? []),
            ]),
          ),
        ];
        const cssModule = this.environment.moduleGraph.getModuleById(resolvedVirtualCssId);
        if (update.cssChanged && cssModule) modules.push(cssModule);
        for (const module of modules) this.environment.moduleGraph.invalidateModule(module);
        if (hasComposition) {
          this.environment.hot.send({ type: "full-reload" });
          return [];
        }
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
    transform: {
      order: "pre",
      async handler(code, id) {
        const sfc = /\.(vue|svelte|astro)$/.test(cleanModuleId(id));
        if (
          (!sfc && !/\.[cm]?[jt]sx?$/.test(cleanModuleId(id))) ||
          (sfc && id !== cleanModuleId(id)) ||
          id.includes("node_modules") ||
          id.endsWith("?stillwind-css-watch") ||
          /[?&](?:raw|url)(?:[=&]|$)/.test(id)
        )
          return undefined;
        const state = environmentState(this.environment?.name);
        if (!/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(code)) {
          state.session?.remove(compilerModulePath(projectRoot, cleanModuleId(id)));
          state.producers.delete(id);
          const removed = cleanModuleId(id).replaceAll("\\", "/");
          state.compositionDependencies.delete(removed);
          state.compositionTopologies.delete(removed);
          return undefined;
        }
        await ensureSession(state, this);
        const session = state.session!;
        // Native needs the import identity before it renders edits. Publish its
        // immutable CSS payload after compilation and before Vite can load it.
        const carrierReference = String(++carrierSequence);
        const allocatedCarrier = `${registeredCssPrefix}${carrierReference}&lang.css`;
        const output = await compileModule(
          session,
          () => (state.typeFacts ??= new TypeScriptFactsService(projectRoot)),
          code,
          id,
          projectRoot,
          {
            resolve: (specifier, importer) => this.resolve(specifier, importer, { skipSelf: true }),
            read: (id) => readFile(id, "utf8"),
            watch: (id) => this.addWatchFile(id),
            warn: (message) => this.warn(message),
          },
          {
            stylesheetImports: sfc ? [virtualCssId, allocatedCarrier] : undefined,
            moduleCssImport: `${moduleCssValuePrefix}${carrierReference}`,
          },
        );
        if (!output?.code) return undefined;
        // TODO: Verify dependent CSS imports for explicit module CSS values, including
        // consumers inserting base and derived stylesheets in different orders.
        if (output.hasModuleCssImport && output.compositionModules.length > 0)
          this.error(
            "Cross-module style composition requires automatic CSS transport; explicit module CSS values currently support same-module composition only.",
          );
        state.producers.add(id);
        const producerId = cleanModuleId(id).replaceAll("\\", "/");
        const previousTopology = state.compositionTopologies.get(producerId);
        state.compositionTopologies.set(producerId, output.compositionTopology);
        if (
          watching &&
          this.environment &&
          "hot" in this.environment &&
          previousTopology !== undefined &&
          previousTopology !== output.compositionTopology
        ) {
          this.environment.hot.send({ type: "full-reload" });
        }
        if (output.compositionTopology !== "[]") {
          state.compositionDependencies.set(
            producerId,
            new Set(
              output.compositionModules.map((module) =>
                resolve(projectRoot, module).replaceAll("\\", "/"),
              ),
            ),
          );
        } else {
          state.compositionDependencies.delete(producerId);
        }
        const upstreamMap = this.getCombinedSourcemap?.();
        const plan = await renderCarrier(state, output.path, {
          placement: output.hasModuleCssImport ? "document" : "carrier",
          resolve: (specifier, importer) => this.resolve(specifier, importer, { skipSelf: true }),
          watch: (id) => this.addWatchFile(id),
          upstream: upstreamMap
            ? { id: cleanModuleId(id), map: JSON.stringify(upstreamMap) }
            : undefined,
        });
        const css = plan.css;
        if (sfc || output.hasModuleCssImport) carrierPayloads.set(carrierReference, css);
        const carrier = sfc ? allocatedCarrier : carrierSpecifier(state, css);
        const finalized = sfc
          ? { code: output.code, removed: [], prefix: "" }
          : finalizeReactModuleWithMap(output.code, false);
        const imports = sfc || output.hasModuleCssImport ? "" : `import "${virtualCssId}";\n`;
        const suffix =
          sfc || output.hasModuleCssImport ? "" : `\nimport ${JSON.stringify(carrier)};\n`;
        return {
          code: imports + finalized.code + suffix,
          meta: {
            stillwind: {
              coverage: plan.coverage,
              designSystem: plan.designSystem,
              producers: plan.producers,
              compositionModules: output.compositionModules,
              compositionTopology: output.compositionTopology,
            },
          },
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
            const playSession = new NativeProjectSession(
              playLoader,
              false,
              undefined,
              options.naming,
            );
            // Preserve byte length for the Play shorthand's source positions.
            const playProject = playSession.compile([
              { path: "Play.tsx", source: source.replace(/\bexport App;/g, "export{App}") },
            ]);
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
