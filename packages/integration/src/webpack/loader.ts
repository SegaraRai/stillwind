import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { cleanModuleId, type UpstreamSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import { compilerOptions, cssEntrypoint, writeVocabulary } from "../options.ts";
import { IntegrationSession } from "../session.ts";
import { cssValueModule, type CssValueResource } from "./css-value.ts";
import { recordProducer, type EmissionContext } from "./emission.ts";
import { createLoaderHost, watchCompilerFiles } from "./host.ts";
import {
  classifyRequest,
  relativeRequest,
  requestOptions,
  virtualRequest,
  type LoaderOptions,
} from "./requests.ts";
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
 * @param source - Source supplied by the preceding loaders. CSS subrequests read the original
 * resource instead.
 * @param inputMap - Map from the supplied source to its original source, when available.
 * @param additionalData - Metadata from preceding loaders, including a prior Stillwind transform
 * fingerprint.
 */
export default function webpackLoader(
  this: LoaderContext<LoaderOptions>,
  source: string,
  inputMap: Parameters<ReturnType<LoaderContext<LoaderOptions>["async"]>>[2],
  additionalData?: {
    stillwind?: {
      sourceTransform?: {
        resource: string;
        fingerprint: string;
      };
    };
    [key: string]: unknown;
  },
) {
  const callback = this.async();
  const text = String(source);
  const { projection, cssValue, sfc, id, passthrough } = classifyRequest(
    this.resourcePath,
    this.resourceQuery,
  );
  const prior = additionalData?.stillwind?.sourceTransform;
  // Framework loaders can clone the same rule into a subrequest's chain.
  // Skip only an identical output already produced for this resource.
  if (prior?.resource === id && prior.fingerprint === fingerprint(text)) {
    return callback(null, source, inputMap, additionalData as Parameters<typeof callback>[3]);
  }
  if (passthrough) {
    return callback(null, source, inputMap);
  }
  if (
    !projection &&
    !cssValue &&
    !/stillwind\/(?:react|preact|solid|astro|svelte|vue|module-css)/.test(text)
  ) {
    return callback(null, source, inputMap);
  }
  const options = requestOptions(this.getOptions());
  if (!options.css && !options.theme) {
    return callback(new Error("Stillwind requires a CSS entrypoint"));
  }
  if (options.naming) {
    this.cacheable(false);
  }
  let compiler: IntegrationSession | undefined;
  const request = (query: string) =>
    virtualRequest(this.resourcePath, sfc, options, import.meta.filename, query);
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
    const host = await import("@stillwind/host");
    watchCompilerFiles(watch);
    const root = path.resolve(this.rootContext, options.root ?? ".");
    const css = path.resolve(root, cssEntrypoint(options));
    const loaded = await host.loadCssGraph(css, compilerOptions(options), {
      resolve: resolveCss,
      read,
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch,
    });
    writeVocabulary(loaded.loader, root, options);
    compiler = new IntegrationSession(
      root,
      loaded.loader,
      options,
      false,
      this.mode === "development" ? "development" : "release",
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
          sfc && !projection && !cssValue && options.automatic !== false
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

    const valueResources: CssValueResource[] = [];
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const owner =
        "moduleId" in resource ? compiler.resolverId(resource.moduleId) : resource.sourceName;
      const resolved = await resolveAsset(resource.specifier, owner);
      if (!resolved) {
        throw new Error(`Cannot resolve Stillwind resource ${resource.specifier} from ${owner}`);
      }
      watch(host.cleanModuleId(resolved));
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
    const automatic = (!output.hasModuleCssImport && options.automatic !== false) || projection;
    if (automatic) {
      const context = this as unknown as EmissionContext;
      recordProducer(context, {
        source: text,
        id,
        root,
        options,
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
    const finalized = host.finalizeModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      sfc || !automatic
        ? finalized.code
        : finalized.code.slice(finalized.prefix.length) + "\n" + finalized.prefix + cssImport;
    const map = host.moduleSourceMap(
      text,
      output.code,
      this.resourcePath,
      output.mappings,
      finalized.removed,
      "",
    );
    const composedMap = host.composeModuleSourceMap(map, inputMap as UpstreamSourceMap);
    if (sfc && this.data) {
      (this.data as { stillwindSourceTransform?: unknown }).stillwindSourceTransform = {
        code,
        map: composedMap,
        resource: id,
      };
    }
    const metadata = {
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
    callback(
      null,
      code,
      composedMap as Parameters<typeof callback>[2],
      metadata as unknown as Parameters<typeof callback>[3],
    );
  })()
    .catch((error) => callback(error))
    .finally(() => compiler?.close());
}

/**
 * Keeps webpack pitch data available for sharing transformed SFC code and maps with framework
 * loaders.
 */
export function pitch() {}
