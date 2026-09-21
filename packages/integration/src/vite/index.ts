import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { dirname, isAbsolute, posix, resolve } from "node:path";
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
  finalizeModuleWithMap as finalizeHostModuleWithMap,
} from "@stillwind/host";
import type { CssUpstreamMap, PlayOutput } from "@stillwind/host";
import { NativeDesignSystemLoader, NativeProjectSession } from "@stillwind/native";
import type { NativeIdentifierNaming } from "@stillwind/native";
import { build, transform as transformCss } from "esbuild";
import type { Plugin, ViteDevServer } from "vite-plus";

import { partitionStylesheet } from "./emission.ts";
import { developmentRuntime } from "./runtime.ts";

/**
 * Project configuration for the Stillwind Vite plugin. `css` or `theme` must identify
 * an entry stylesheet; `css` takes precedence when both are supplied. Relative paths
 * are resolved against Vite's project root.
 */
export interface StillwindOptions {
  /** Apply producer CSS to the document. Disable for libraries that supply explicit CSS values. */
  automatic?: boolean;
  /** Build transport. Split outputs use the same ordered manifest in the browser and SSR. */
  emission?: "single" | "split";
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
  /** Reject Tailwind group/peer/container relationships in favor of typed refs. */
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
  /**
   * Base-36 digits a built-in generated name keeps from its request hash. Defaults to 5;
   * values from 1 to 50 are accepted. Raising it lowers the chance of a collision.
   */
  hashLength?: number;
}

const virtualCssId = "virtual:stillwind.css";
const maximumPlayPayloadBytes = 1_000_000;

function resourceUrl(file: string): string {
  return `/@fs/${file
    .replaceAll("\\", "/")
    .split("/")
    .map((segment) =>
      encodeURIComponent(segment)
        .replaceAll("'", "%27")
        .replaceAll("(", "%28")
        .replaceAll(")", "%29"),
    )
    .join("/")}`;
}

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

/// Whether one module id is a stylesheet the host authored, rather than one Stillwind generated.
///
/// The design system's own sources are reserved already, so reserving them again changes nothing;
/// the carriers the plugin serves are generated CSS and must never be read as authored names.
function isAuthoredStylesheet(id: string): boolean {
  if (!cleanModuleId(id).endsWith(".css")) return false;
  if (id.startsWith("virtual:") || id.includes("?")) return false;
  const path = cleanModuleId(id).replaceAll("\\", "/");
  return !path.startsWith("\0stillwind:");
}

function compilerOptions(options: StillwindOptions) {
  return {
    prefix: options.prefix ?? "--sw-",
    tailwindPrefix: options.tailwindPrefix ?? "--tw-",
    forbidTailwindRelationships: options.forbidTailwindRelationships ?? false,
    classNamespace: options.classNamespace ?? "",
    useCssIf: options.useCssIf ?? false,
    namingRevision: options.namingRevision ?? "",
    hashLength: options.hashLength,
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
  const withRuntime = finalizeModule(code, false);
  return withRuntime.replace(/\bexport\s+App\s*;/g, "export { App };");
}

export function finalizeModule(code: string, includeStylesheet = true) {
  return finalizeModuleWithMap(code, includeStylesheet).code;
}
export function finalizeModuleWithMap(code: string, includeStylesheet = true) {
  return finalizeHostModuleWithMap(code, includeStylesheet ? virtualCssId : undefined);
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
    work: Promise<void>;
    loadRevision: number;
    session?: NativeProjectSession;
    initializing?: Promise<void>;
    resources: Map<string, string>;
    cssFiles: Set<string>;
    typeFacts?: TypeScriptFactsService;
    revision: number;
    buildCss?: string;
    manifest?: {
      version: 1;
      revision: string;
      parts: { id: string; href: string }[];
      modules: Record<string, string[]>;
    };
  };
  const environments = new Map<string, EnvironmentState>();
  const environmentState = (name = "client") => {
    let state = environments.get(name);
    if (!state) {
      state = {
        work: Promise.resolve(),
        loadRevision: 0,
        resources: new Map(),
        cssFiles: new Set(),
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
      const url = resourceUrl(resolved);
      references.set(resource.placeholder, JSON.stringify(url).slice(1, -1));
      dependencies.add(cleanModuleId(resolved));
    }
    loaderResources.set(loader, references);
    return loader;
  };

  const writeTypes = (loader: NativeDesignSystemLoader) => {
    if (options.types === false) return;
    const path = resolve(projectRoot, options.types ?? "src/stillwind.generated.d.ts");
    const types = loader.generateVocabularyTypes();
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
        for (const path of dependencies) context.addWatchFile(path);
        writeTypes(loader);
      };
      state.initializing = initialize().finally(() => {
        state.initializing = undefined;
      });
    }
    await state.initializing;
  };

  type Input = {
    projection?: boolean;
    upstream?: CssUpstreamMap;
    code: string;
    id: string;
    path: string;
    manual: boolean;
    javascript: string;
    dependencies: string[];
  };
  const inputs = new Map<string, Map<string, Input>>();
  const inputMap = (name = "client") => {
    let map = inputs.get(name);
    if (!map) inputs.set(name, (map = new Map()));
    return map;
  };
  const includeSourceContent = (name?: string) =>
    ![...inputMap(name).values()].some((input) => input.projection);
  let devServerRef: ViteDevServer | undefined;
  let assetsDirectory = "assets";
  let publicBase = "/";
  let sourceMaps = false;
  let devSourceMaps = false;
  let minifyCss = true;
  const fragments = new Map<string, string>();
  const fragmentSpecifier = (css: string) => {
    const key = watching
      ? createHash("sha256").update(css).digest("hex")
      : Buffer.from(css).toString("base64url");
    if (watching) fragments.set(key, css);
    return "virtual:stillwind/fragment:" + key;
  };
  const runtimeId = "\0stillwind:runtime";
  const valuePrefix = "\0stillwind:value:";
  const fragmentPrefix = "\0stillwind:fragment:";
  const projectionPrefix = "\0stillwind:projection:";
  const producerPrefix = "\0stillwind:producer:";
  const runtimeSpecifier = "virtual:stillwind/runtime";
  const producerSpecifier = (id: string) =>
    `virtual:stillwind/producer:${Buffer.from(compilerModulePath(projectRoot, cleanModuleId(id))).toString("base64url")}`;
  const valueSpecifier = (id: string) =>
    `virtual:stillwind/value:${Buffer.from(cleanModuleId(id)).toString("base64url")}`;
  const resolvePlan = async (
    state: EnvironmentState,
    modules: string[],
    context: {
      resolve: (
        specifier: string,
        importer: string,
        options: { skipSelf: boolean },
      ) => Promise<{ id: string; external?: boolean | string } | null>;
      addWatchFile: (id: string) => void;
    },
    complete = false,
  ) => {
    const revision = state.revision;
    const plan = state.session!.planStylesheet({
      modules,
      coverage: complete
        ? { kind: "complete", target: "application", revision: String(state.revision) }
        : { kind: "observed" },
      placement: "document",
    });
    const resources = new Map(state.resources);
    for (const resource of plan.resources) {
      const resolved = await context.resolve(
        resource.specifier,
        resolve(projectRoot, resource.owner),
        { skipSelf: true },
      );
      const file = resolvedFile(resolved, `Stillwind resource ${resource.specifier}`);
      context.addWatchFile(file);
      resources.set(resource.placeholder, JSON.stringify(resourceUrl(file)).slice(1, -1));
    }
    return {
      ...plan,
      revision,
      originalCss: plan.css,
      resourceReplacements: resources,
      css: resolveResources(plan.css, resources),
      rawParts: plan.parts,
      parts: plan.parts.map((part) => {
        const name = [...environments].find(([, candidate]) => candidate === state)?.[0];
        const mapped =
          watching && devSourceMaps
            ? decodeCssSourceMap(
                cssWithSourceMap(
                  part,
                  resources,
                  projectRoot,
                  [...inputMap(name).values()].flatMap((input) =>
                    input.upstream ? [input.upstream] : [],
                  ),
                  includeSourceContent(name),
                ),
              )
            : undefined;
        return {
          key: part.key,
          producers: part.producers,
          css: mapped?.code ?? resolveResources(part.css, resources),
          map: mapped?.map,
        };
      }),
    };
  };
  const documentPlan = async (
    name: string | undefined,
    context: Parameters<typeof resolvePlan>[2],
    complete = false,
  ) => {
    const state = environmentState(name);
    await ensureSession(state, context);
    const modules = [...inputMap(name).values()]
      .filter((input) => !input.manual)
      .map((input) => input.path);
    return resolvePlan(state, modules, context, complete);
  };
  const publish = async (name: string | undefined, context: Parameters<typeof resolvePlan>[2]) => {
    if (!watching || !devServerRef) return;
    const state = environmentState(name);
    const revision = ++state.revision;
    const plan = await documentPlan(name, context);
    if (revision === state.revision)
      devServerRef.environments[name ?? "client"]?.hot.send({
        type: "custom",
        event: "stillwind:css",
        data: {
          revision,
          parts: plan.parts,
          dependencies: Object.fromEntries(
            [...inputMap(name).values()].map((input) => [input.path, input.dependencies]),
          ),
        },
      });
  };
  const compile = async (
    name: string | undefined,
    code: string,
    id: string,
    context: Parameters<typeof resolvePlan>[2] & {
      warn: (message: string) => void;
      getCombinedSourcemap?: () => unknown;
    },
  ) => {
    const state = environmentState(name);
    const previous = state.work;
    let release!: () => void;
    state.work = new Promise<void>((resolve) => {
      release = resolve;
    });
    await previous;
    try {
      await ensureSession(state, context);
      const sfc = /\.(vue|svelte|astro)$/.test(cleanModuleId(id));
      const output = await compileModule(
        state.session!,
        () => (state.typeFacts ??= new TypeScriptFactsService(projectRoot)),
        code,
        id,
        projectRoot,
        {
          resolve: (specifier, importer) =>
            context.resolve(specifier, importer, { skipSelf: true }),
          read: (id) => readFile(id, "utf8"),
          watch: (id) => context.addWatchFile(id),
          warn: (message) => context.warn(message),
        },
        {
          stylesheetImports:
            sfc && options.automatic !== false ? [producerSpecifier(id)] : undefined,
          moduleCssImport: valueSpecifier(id),
        },
      );
      if (!output?.code) return;
      const finalized = sfc
        ? { code: output.code, removed: [], prefix: "" }
        : finalizeModuleWithMap(output.code, false);
      const suffix =
        sfc || output.hasModuleCssImport || options.automatic === false
          ? ""
          : `\nimport ${JSON.stringify(producerSpecifier(id))};\n`;
      const javascript = finalized.code + suffix;
      inputMap(name).set(cleanModuleId(id), {
        projection: inputMap(name).get(cleanModuleId(id))?.projection,
        upstream: context.getCombinedSourcemap
          ? { id: cleanModuleId(id), map: JSON.stringify(context.getCombinedSourcemap()) }
          : inputMap(name).get(cleanModuleId(id))?.upstream,
        code,
        id,
        path: output.path,
        manual: output.hasModuleCssImport || options.automatic === false,
        javascript,
        dependencies: output.compositionModules,
      });
      return {
        code: javascript,
        map: JSON.stringify(
          moduleSourceMap(
            code,
            output.code,
            id,
            output.mappings,
            finalized.removed,
            finalized.prefix,
          ),
        ),
        meta: {
          stillwind: { input: code, id, path: output.path, manual: output.hasModuleCssImport },
        },
      };
    } finally {
      release();
    }
  };

  return {
    name: "stillwind",
    enforce: "pre",
    perEnvironmentStartEndDuringDev: true,
    perEnvironmentWatchChangeDuringDev: true,
    async resolveId(id, importer) {
      if (id.endsWith("?stillwind.css")) {
        const source = await this.resolve(id.slice(0, -"?stillwind.css".length), importer, {
          skipSelf: true,
        });
        if (!source || source.external) this.error(`Cannot resolve Stillwind projection ${id}`);
        return projectionPrefix + Buffer.from(cleanModuleId(source.id)).toString("base64url");
      }
      if (id === runtimeSpecifier || id === virtualCssId) return runtimeId;
      if (id === "virtual:stillwind/snapshot.css") return "\0stillwind:snapshot.css";
      if (id.startsWith("virtual:stillwind/value:"))
        return valuePrefix + id.slice("virtual:stillwind/value:".length);
      if (id.startsWith("virtual:stillwind/fragment:"))
        return fragmentPrefix + id.slice("virtual:stillwind/fragment:".length);
      if (id.startsWith("virtual:stillwind/producer:"))
        return producerPrefix + id.slice("virtual:stillwind/producer:".length);
      if (id.startsWith("\0stillwind:")) return id;
      return undefined;
    },
    async load(id) {
      const name = this.environment?.name;
      if (id === "\0stillwind:snapshot.css") {
        const plan = await documentPlan(name, this);
        return decodeCssSourceMap(
          cssWithSourceMap(
            { css: plan.originalCss, sourceMap: plan.sourceMap },
            plan.resourceReplacements,
            projectRoot,
            [...inputMap(name).values()].flatMap((input) =>
              input.upstream ? [input.upstream] : [],
            ),
            includeSourceContent(name),
          ),
        );
      }
      if (id.startsWith(producerPrefix)) {
        if (!watching) return "export {};";
        const producer = Buffer.from(id.slice(producerPrefix.length), "base64url").toString("utf8");
        return `import {register,unregister,synchronize} from ${JSON.stringify(runtimeSpecifier)}; await synchronize(${environmentState(name).revision}); register(${JSON.stringify(producer)}); if(import.meta.hot) import.meta.hot.prune(()=>unregister(${JSON.stringify(producer)}));`;
      }
      if (id.startsWith(projectionPrefix)) {
        const source = Buffer.from(id.slice(projectionPrefix.length), "base64url").toString("utf8");
        this.addWatchFile(source);
        const result = await compile(name, await readFile(source, "utf8"), source, this);
        const input = inputMap(name).get(source);
        if (input) {
          input.manual = false;
          input.projection = true;
        }
        await publish(name, this);
        return {
          code: `import ${JSON.stringify(producerSpecifier(source))};`,
          meta: { stillwind: { ...result?.meta.stillwind, projection: true } },
        };
      }
      if (id === runtimeId) {
        if (!watching) return "export {};";
        const plan = await documentPlan(name, this);
        const revision = environmentState(name).revision;
        const code = developmentRuntime(
          {
            revision,
            parts: plan.parts,
            dependencies: Object.fromEntries(
              [...inputMap(name).values()].map((input) => [input.path, input.dependencies]),
            ),
          },
          devSourceMaps,
          `${publicBase}__stillwind/css?environment=${encodeURIComponent(name ?? "client")}`,
        );
        if (!devSourceMaps) return code;
        const bundled = await build({
          stdin: { contents: code, resolveDir: dirname(fileURLToPath(import.meta.url)) },
          bundle: true,
          write: false,
          format: "esm",
          platform: "browser",
        });
        return bundled.outputFiles[0].text;
      }
      if (id.startsWith(fragmentPrefix)) {
        const key = id.slice(fragmentPrefix.length);
        let css = watching ? fragments.get(key) : Buffer.from(key, "base64url").toString("utf8");
        if (css === undefined) this.error(`Unknown Stillwind CSS fragment ${key}`);
        if (!watching && minifyCss)
          css = (await transformCss(css, { loader: "css", minify: true })).code;
        const files = [
          ...new Set([...css.matchAll(/\/@fs\/([^"'\s)]+)/g)].map((match) => match[1])),
        ];
        const imports = files
          .map(
            (file, index) =>
              `import asset${index} from ${JSON.stringify(decodeURIComponent(file) + "?url")};`,
          )
          .join("\n");
        const replacements = files
          .map(
            (file, index) =>
              `.replaceAll(${JSON.stringify(`/@fs/${file}`)}, JSON.stringify(asset${index}).slice(1,-1))`,
          )
          .join("");
        return `${imports}\nexport const css = ${JSON.stringify(css)}${replacements};`;
      }
      if (id.startsWith(valuePrefix)) {
        const sourceId = Buffer.from(id.slice(valuePrefix.length), "base64url").toString("utf8");
        this.addWatchFile(sourceId);
        if (!inputMap(name).has(sourceId))
          await compile(name, await readFile(sourceId, "utf8"), sourceId, this);
        const input = inputMap(name).get(sourceId);
        if (!input) this.error(`No Stillwind producer for ${sourceId}`);
        const plan = await resolvePlan(environmentState(name), [input.path], this, !watching);
        for (const producer of plan.producers)
          this.addWatchFile(resolve(projectRoot, producer.module));
        for (const file of environmentState(name).cssFiles) this.addWatchFile(file);
        const imports = plan.parts
          .map(
            (part, index) =>
              `import { css as p${index} } from ${JSON.stringify(fragmentSpecifier(part.css))};`,
          )
          .join("\n");
        return `${imports}\nexport const parts = [${plan.parts.map((part, index) => `{key:${JSON.stringify(part.key)},css:p${index}}`).join(",")}];\nexport const css = parts.map(part => part.css).join("");\nexport default css;`;
      }
      return undefined;
    },
    configResolved(config) {
      projectRoot = config.root;
      assetsDirectory = config.build.assetsDir;
      publicBase = config.base;
      sourceMaps = Boolean(config.build.sourcemap);
      devSourceMaps = Boolean(config.css.devSourcemap);
      minifyCss = config.build.cssMinify !== false;
      watching = config.command === "serve";
      compilerMode = watching ? "development" : "release";
      closeTypeFacts();
    },
    async buildStart() {
      const name = this.environment?.name;
      const state = environmentState(name);
      state.session = undefined;
      inputMap(name).clear();
      state.buildCss = undefined;
      state.manifest = undefined;
      await ensureSession(state, this);
    },
    shouldTransformCachedModule({ id }) {
      return (
        /\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(cleanModuleId(id)) &&
        !id.includes("node_modules")
      );
    },
    async renderStart() {
      if (watching) return;
      const name = this.environment?.name;
      const state = environmentState(name);
      // The final graph includes lazy modules and cached transforms. Recover inputs
      // from graph metadata before making a complete-coverage claim.
      for (const id of this.getModuleIds()) {
        const metadata = this.getModuleInfo(id)?.meta.stillwind;
        if (metadata?.input !== undefined && !inputMap(name).has(cleanModuleId(metadata.id ?? id)))
          await compile(name, metadata.input, metadata.id ?? id, this);
        if (metadata?.projection) {
          const input = inputMap(name).get(cleanModuleId(metadata.id ?? id));
          if (input) input.projection = true;
        }
      }
      const plan = await documentPlan(name, this, true);
      if (
        ![...this.getModuleIds()].some(
          (id) =>
            id === runtimeId || id.startsWith(producerPrefix) || id.startsWith(projectionPrefix),
        )
      )
        return;
      const assets = new Map<string, string>();
      let css = plan.css;
      for (const match of css.matchAll(/\/@fs\/([^"'\s)]+)/g)) {
        const file = match[1];
        if (assets.has(file)) continue;
        const reference = this.emitFile({
          type: "asset",
          name: decodeURIComponent(file).split("/").at(-1),
          source: await readFile(decodeURIComponent(file)),
        });
        assets.set(file, reference);
      }
      const resourceUrls = (text: string) => {
        for (const [file, reference] of assets)
          text = text.replaceAll(
            `/@fs/${file}`,
            posix.relative(assetsDirectory, this.getFileName(reference)),
          );
        return text;
      };
      css = resourceUrls(css);
      const emitCss = async (text: string, sourceMap?: string) => {
        if (minifyCss) {
          const input = sourceMap
            ? `${text}\n/*# sourceMappingURL=data:application/json;base64,${Buffer.from(sourceMap).toString("base64")} */`
            : text;
          const result = await transformCss(input, {
            loader: "css",
            minify: true,
            sourcemap: sourceMap ? "external" : false,
            sourcefile: "stillwind.css",
          });
          text = result.code;
          sourceMap = result.map || undefined;
        }
        const hash = createHash("sha256").update(text).digest("hex").slice(0, 16);
        if (sourceMap) {
          this.emitFile({
            type: "asset",
            fileName: `${assetsDirectory}/stillwind-${hash}.css.map`,
            source: sourceMap,
          });
          text += `\n/*# sourceMappingURL=stillwind-${hash}.css.map */`;
        }
        return this.emitFile({
          type: "asset",
          fileName: `${assetsDirectory}/stillwind-${hash}.css`,
          source: text,
        });
      };
      const href = (reference: string) => publicBase + this.getFileName(reference);
      const revision = createHash("sha256")
        .update(plan.designSystem)
        .update(plan.originalCss)
        .update(JSON.stringify(plan.producers))
        .digest("hex");
      if (options.emission !== "split") {
        const replacements = new Map(
          [...plan.resourceReplacements].map(([key, value]) => [key, resourceUrls(value)]),
        );
        const mapped = sourceMaps
          ? decodeCssSourceMap(
              cssWithSourceMap(
                { css: plan.originalCss, sourceMap: plan.sourceMap },
                replacements,
                projectRoot,
                [...inputMap(name).values()].flatMap((input) =>
                  input.upstream ? [input.upstream] : [],
                ),
                includeSourceContent(name),
              ),
            )
          : undefined;
        state.buildCss = await emitCss(css, mapped?.map ? JSON.stringify(mapped.map) : undefined);
        state.manifest = {
          version: 1,
          revision,
          parts: [{ id: "document", href: href(state.buildCss) }],
          modules: Object.fromEntries(
            plan.producers.map((producer) => [producer.module, ["document"]]),
          ),
        };
      } else {
        const references = new Map<string, string>();
        const parts: { id: string; href: string }[] = [];
        const partitions = partitionStylesheet(plan.rawParts);
        for (const [index, part] of partitions.entries()) {
          let reference = references.get(part.key);
          if (!reference) {
            const replacements = new Map(
              [...plan.resourceReplacements].map(([key, value]) => [key, resourceUrls(value)]),
            );
            const mapped = sourceMaps
              ? decodeCssSourceMap(
                  cssWithSourceMap(
                    part,
                    replacements,
                    projectRoot,
                    [...inputMap(name).values()].flatMap((input) =>
                      input.upstream ? [input.upstream] : [],
                    ),
                    includeSourceContent(name),
                  ),
                )
              : undefined;
            references.set(
              part.key,
              (reference = await emitCss(
                mapped?.code ?? resolveResources(part.css, replacements),
                mapped?.map ? JSON.stringify(mapped.map) : undefined,
              )),
            );
          }
          parts.push({ id: `${index}:${part.key}`, href: href(reference) });
        }
        const dependencies = new Map(
          [...inputMap(name).values()].map((input) => [input.path, input.dependencies]),
        );
        const modules = Object.fromEntries(
          plan.producers.map((producer) => {
            const closure = new Set([producer.module]);
            for (const module of closure)
              for (const dependency of dependencies.get(module) ?? []) closure.add(dependency);
            return [
              producer.module,
              parts
                .filter(
                  (_, index) =>
                    partitions[index].producers.length === 0 ||
                    partitions[index].producers.some((owner) => closure.has(owner)),
                )
                .map((part) => part.id),
            ];
          }),
        );
        state.manifest = { version: 1, revision, parts, modules };
      }
      this.emitFile({
        type: "asset",
        fileName: "stillwind-manifest.json",
        source: JSON.stringify(state.manifest, null, 2),
      });
    },
    renderChunk(_code, chunk) {
      const { manifest, buildCss } = environmentState(this.environment?.name);
      if (options.emission === "split" && manifest) {
        const graph = new Set(Object.keys(chunk.modules));
        if (chunk.facadeModuleId) graph.add(chunk.facadeModuleId);
        for (const id of graph)
          for (const dependency of this.getModuleInfo(id)?.importedIds ?? []) graph.add(dependency);
        const ids = new Set(
          [...graph].flatMap(
            (id) =>
              manifest!.modules[
                inputMap(this.environment?.name).get(cleanModuleId(id))?.path ??
                  this.getModuleInfo(id)?.meta.stillwind?.path ??
                  (id.startsWith(producerPrefix)
                    ? Buffer.from(id.slice(producerPrefix.length), "base64url").toString("utf8")
                    : "")
              ] ?? [],
          ),
        );
        const parts = manifest.parts
          .filter((part) => ids.has(part.id))
          .map((part) => ({
            ...part,
            order: manifest!.parts.indexOf(part),
            relative: publicBase === "./" || publicBase === "",
            href:
              publicBase === "./" || publicBase === ""
                ? posix.relative(posix.dirname(chunk.fileName), part.href.slice(publicBase.length))
                : part.href,
          }));
        if (parts.length)
          return {
            code: `${_code}\nif(typeof document!=="undefined"){for(const p of ${JSON.stringify(parts)}){const links=[...document.querySelectorAll('link[data-stillwind-part]')];if(links.some(l=>l.dataset.stillwindPart===p.id))continue;const l=document.createElement('link');l.rel='stylesheet';l.href=p.relative?new URL(p.href,import.meta.url).href:p.href;l.dataset.stillwindPart=p.id;l.dataset.stillwindOrder=String(p.order);document.head.insertBefore(l,links.find(e=>Number(e.dataset.stillwindOrder)>p.order)??null);}}`,
            map: null,
          };
      }
      if (buildCss) {
        const metadata = chunk as typeof chunk & {
          viteMetadata?: { importedCss: Set<string>; importedAssets: Set<string> };
        };
        metadata.viteMetadata ??= {
          importedCss: new Set(),
          importedAssets: new Set(),
          __modules: new Set(),
        };
        metadata.viteMetadata.importedCss.add(this.getFileName(buildCss));
      }
      return null;
    },
    async hotUpdate(context) {
      const name = this.environment.name;
      const state = environmentState(name);
      const id = cleanModuleId(context.file);
      const host = {
        resolve: (specifier: string, importer: string) =>
          this.environment.pluginContainer.resolveId(specifier, importer),
        addWatchFile: (id: string) => {
          context.server.watcher.add(id);
        },
        warn: (message: string) => this.warn(message),
      };
      try {
        if (state.cssFiles.has(context.file) || state.cssFiles.has(resolve(context.file))) {
          const revision = ++state.loadRevision;
          const dependencies = new Set<string>();
          const loader = await loadDesignSystem(
            async (specifier, importer) =>
              resolvedFile(await host.resolve(specifier, importer), `CSS ${specifier}`),
            undefined,
            dependencies,
          );
          if (revision !== state.loadRevision) return [];
          const update = state.session!.reload(loader);
          state.resources = loaderResources.get(loader) ?? new Map();
          state.cssFiles = dependencies;
          context.server.watcher.add([...dependencies]);
          writeTypes(loader);
          await publish(name, host);
          const changed = new Set(update.transformedModules);
          const modules = [...inputMap(name).values()]
            .filter((input) => input.manual || changed.has(input.path))
            .flatMap((input) => [
              ...(this.environment.moduleGraph.getModulesByFile(cleanModuleId(input.id)) ?? []),
              ...(input.manual
                ? [
                    this.environment.moduleGraph.getModuleById(
                      valuePrefix + Buffer.from(cleanModuleId(input.id)).toString("base64url"),
                    ),
                  ].filter((module): module is NonNullable<typeof module> => Boolean(module))
                : []),
            ]);
          for (const module of modules) this.environment.moduleGraph.invalidateModule(module);
          return modules;
        }
        const previous = inputMap(name).get(id);
        if (previous && context.type !== "delete") {
          const result = await compile(name, await context.read(), id, host);
          await publish(name, host);
          if (!previous.manual && result?.code === previous.javascript) return [];
        }
      } catch (error) {
        this.warn(`Stillwind update failed; retaining published styles: ${String(error)}`);
      }
      return undefined;
    },
    async watchChange(id, change) {
      if (change.event !== "delete") return;
      const name = this.environment?.name;
      const state = environmentState(name);
      if (!state.session || !inputMap(name).delete(cleanModuleId(id))) return;
      state.session.remove(compilerModulePath(projectRoot, cleanModuleId(id)));
      await publish(name, this);
    },
    transform: {
      order: "pre",
      async handler(code, id) {
        const name = this.environment?.name;
        if (isAuthoredStylesheet(id)) {
          const state = environmentState(name);
          await ensureSession(state, this);
          state.session!.reserveAuthoredCss(
            compilerModulePath(projectRoot, cleanModuleId(id)),
            code,
          );
          return;
        }
        if (
          !/\.(?:[cm]?[jt]sx?|vue|svelte|astro)$/.test(cleanModuleId(id)) ||
          id.includes("node_modules") ||
          id.startsWith("\0") ||
          /[?&](?:raw|url|type)(?:[=&]|$)/.test(id)
        )
          return;
        if (!/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(code)) {
          if (inputMap(name).delete(cleanModuleId(id))) {
            environmentState(name).session?.remove(
              compilerModulePath(projectRoot, cleanModuleId(id)),
            );
            await publish(name, this);
          }
          return;
        }
        const result = await compile(name, code, id, this);
        await publish(name, this);
        return result;
      },
    },
    configureServer(server: ViteDevServer) {
      devServerRef = server;
      server.middlewares.use("/__stillwind/css", (request, response) => {
        const name =
          new URL(request.url ?? "", "http://stillwind.local").searchParams.get("environment") ??
          "client";
        const environment = server.environments[name];
        if (!environment) return sendJson(response, { error: "Unknown environment" }, 404);
        const host = {
          resolve: (specifier: string, importer: string) =>
            environment.pluginContainer.resolveId(specifier, importer),
          addWatchFile: (id: string) => {
            server.watcher.add(id);
          },
        };
        void documentPlan(name, host)
          .then((plan) =>
            sendJson(response, {
              revision: plan.revision,
              parts: plan.parts,
              dependencies: Object.fromEntries(
                [...inputMap(name).values()].map((input) => [input.path, input.dependencies]),
              ),
            }),
          )
          .catch((error) => sendJson(response, { error: String(error) }, 500));
      });
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
