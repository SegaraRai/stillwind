import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { resolveIntegrationConfig } from "@stillwind/config/node";
import {
  cleanModuleId,
  composeModuleSourceMap,
  cssEntrypoint,
  finalizeModuleWithMap,
  loadCssGraph,
  moduleSourceMap,
  nodePluginHost,
  warnRejectedNames,
  type UpstreamSourceMap,
} from "@stillwind/host";

import { buildCompilerOptions, IntegrationSession } from "../session.ts";
import { cssValueModule, type CssValueResource } from "./css-value.ts";
import { recordProducer } from "./emission.ts";
import { createLoaderHost, watchCompilerFiles } from "./host.ts";
import {
  classifyRequest,
  relativeRequest,
  requestOptions,
  virtualRequest,
  type LoaderOptions,
} from "./requests.ts";
import {
  loaderCompletion,
  type LoaderAdditionalData,
  type SourceTransformContext,
} from "./source-map-loader.ts";
import { sourceDescriptor } from "./sources.ts";

/**
 * Hashes transformed text to recognize a repeated pass through the same resource.
 *
 * @param source - Exact source text received or produced by the loader.
 * @returns A SHA-256 fingerprint encoded as hexadecimal.
 */
const fingerprint = (source: string) => createHash("sha256").update(source).digest("hex");

/**
 * Compiles a producer, CSS projection, or explicit CSS value through the webpack loader protocol.
 *
 * The loader context supplies resolution, file dependencies, options, and an asynchronous callback.
 * Automatic document CSS and projections require `StillwindPlugin` to be installed before
 * compilation. Results and failures are delivered through the callback. SFC transforms also store
 * their code and map in `this.data` for framework map composition.
 *
 * @param this - Active loader context whose data and module metadata retain successful producer transforms.
 * @param source - Source supplied by the preceding loaders. CSS subrequests read the original
 * resource instead.
 * @param inputMap - Map from the supplied source to its original source, when available.
 * @param additionalData - Metadata from preceding loaders, including a prior Stillwind transform
 * fingerprint.
 * @returns No synchronous source. Pass-through and asynchronous results use the loader callback.
 */
export default function webpackLoader(
  this: SourceTransformContext<LoaderOptions>,
  source: string,
  inputMap: UpstreamSourceMap,
  additionalData?: LoaderAdditionalData,
) {
  const callback = loaderCompletion(this);
  const text = String(source);
  const { projection, cssValue, sfc, id, passthrough } = classifyRequest(
    this.resourcePath,
    this.resourceQuery,
  );
  const prior = additionalData?.stillwind?.sourceTransform;
  // Framework loaders can clone the same rule into a subrequest's chain.
  // Skip only an identical output already produced for this resource.
  if (prior?.resource === id && prior.fingerprint === fingerprint(text)) {
    return callback(null, source, inputMap, additionalData);
  }
  if (passthrough) {
    return callback(null, source, inputMap);
  }
  if (
    !projection &&
    !cssValue &&
    !/stillwind\/(?:alpine|astro|lit|preact|qwik|react|solid|svelte|vanilla|vue|module-css)/.test(
      text,
    )
  ) {
    return callback(null, source, inputMap);
  }
  const supplied = requestOptions(this.getOptions());
  let compiler: IntegrationSession | undefined;
  const {
    read,
    readDependency,
    dependencySources,
    resolveCss,
    resolveAsset,
    resolve,
    watch,
    warn,
  } = createLoaderHost(this, import.meta.filename);
  void (async () => {
    const configuration =
      this._compilation?.__stillwindConfiguration ??
      (await resolveIntegrationConfig(supplied, this.rootContext));
    for (const file of configuration.dependencies) {
      this.addDependency(file);
    }
    for (const file of configuration.missing) {
      this.addMissingDependency(file);
    }
    const { config } = configuration;
    if (config.naming) {
      this.cacheable(false);
    }
    /**
     * Creates a virtual module request using this source file's framework and compiler settings.
     * @param query - Stillwind virtual query selecting the requested CSS or value module.
     * @returns An inline loader request retaining the resource path and resolved configuration.
     */
    const request = (query: string) =>
      virtualRequest(this.resourcePath, sfc, config, import.meta.filename, query, supplied);
    watchCompilerFiles(watch);
    const root = path.resolve(this.rootContext, config.root ?? ".");
    const css = path.resolve(root, cssEntrypoint(config));
    const loaded = await loadCssGraph(
      css,
      buildCompilerOptions(config),
      {
        resolve: resolveCss,
        read,
        documentUri: (id) => pathToFileURL(id).href,
        sourceMapSource: (id) => pathToFileURL(id).href,
        watch,
        watchDirectory: (id) => this.addContextDependency(id),
        plugins: nodePluginHost,
      },
      undefined,
      undefined,
      {
        plugins: config.plugins,
        tailwind: config.tailwind,
        important: config.important,
        root,
      },
    );
    compiler = new IntegrationSession(
      root,
      loaded.loader,
      config,
      false,
      this.mode === "development" ? "development" : "release",
      loaded.plugins,
    );
    const session = compiler.native;
    const output = await compiler.compile(
      projection || cssValue ? await read(this.resourcePath) : text,
      projection || cssValue ? this.resourcePath : id,
      {
        describe: (id) => sourceDescriptor(root, id),
        resolve,
        read: readDependency,
        watch,
        warn,
      },
      {
        stylesheetImports:
          sfc && !projection && !cssValue && config.automatic !== false
            ? [request("stillwind.css")]
            : undefined,
        moduleCssImport: request("stillwind-module-css"),
      },
    );
    if (!output?.code) {
      return callback(null, projection ? "" : source, projection ? null : inputMap);
    }
    const plan = session.planStylesheet({
      modules: [output.moduleId],
      coverage: { kind: "observed" },
      placement: "document",
    });
    warnRejectedNames(plan.rejectedNames, warn);

    const valueResources: CssValueResource[] = [];
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const owner =
        "moduleId" in resource ? compiler.resolverId(resource.moduleId) : resource.sourceName;
      const resolved = await resolveAsset(resource.specifier, owner);
      if (!resolved) {
        throw new Error(`Cannot resolve Stillwind resource ${resource.specifier} from ${owner}`);
      }
      watch(cleanModuleId(resolved));
      if (plan.css.includes(resource.placeholder)) {
        valueResources.push({
          placeholder: resource.placeholder,
          id: resolved,
          request: relativeRequest(path.dirname(this.resourcePath), resolved),
        });
      }
    }
    const carrier = relativeRequest(
      path.dirname(this.resourcePath),
      fileURLToPath(
        new URL("./stillwind-webpack.css", import.meta.resolve("@stillwind/integration/webpack")),
      ),
    );
    if (cssValue) {
      return callback(null, cssValueModule(plan.parts, valueResources), null);
    }
    const automatic = (!output.hasModuleCssImport && config.automatic !== false) || projection;
    if (automatic) {
      recordProducer(this, {
        source: text,
        id,
        root,
        config: config,
        manual: false,
        upstream: inputMap
          ? {
              id: this.resourcePath,
              sourceMapSource: sourceDescriptor(root, id).document.uri,
              map: typeof inputMap === "string" ? inputMap : JSON.stringify(inputMap),
            }
          : undefined,
        dependencies: await Promise.all(
          [...dependencySources].map(async ([id, pending]) => {
            const { source, map } = await pending;
            return {
              id,
              source,
              upstream: map
                ? {
                    id: cleanModuleId(id),
                    sourceMapSource: sourceDescriptor(root, id).document.uri,
                    map: typeof map === "string" ? map : JSON.stringify(map),
                  }
                : undefined,
            };
          }),
        ),
      });
    }
    const cssImport = `import ${JSON.stringify(carrier + "?stillwind-document")};\n`;
    if (projection) {
      return callback(null, cssImport);
    }
    const finalized = finalizeModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      sfc || !automatic
        ? finalized.code
        : finalized.code.slice(finalized.prefix.length) + "\n" + finalized.prefix + cssImport;
    const map = moduleSourceMap(
      text,
      output.code,
      this.resourcePath,
      output.mappings,
      finalized.removed,
      "",
    );
    const composedMap = composeModuleSourceMap(map, inputMap);
    if (sfc && this.data) {
      this.data.stillwindSourceTransform = {
        code,
        map: composedMap,
        resource: id,
      };
    }
    const metadata: LoaderAdditionalData = {
      ...additionalData,
      stillwind: {
        coverage: plan.coverage,
        designSystem: plan.designSystem,
        producers: plan.producers,
        sourceTransform: {
          resource: id,
          fingerprint: fingerprint(code),
        },
      },
    };
    callback(null, code, composedMap, metadata);
  })()
    .catch((error) => callback(error))
    .finally(() => compiler?.close());
}

/**
 * Keeps webpack pitch data available for sharing transformed SFC code and maps with framework
 * loaders.
 */
export function pitch() {}
