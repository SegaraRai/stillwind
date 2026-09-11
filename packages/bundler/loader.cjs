const path = require("node:path");
const fs = require("node:fs");
const { isBuiltin } = require("node:module");
const { createHash } = require("node:crypto");
const fingerprint = (source) => createHash("sha256").update(source).digest("hex");

const carrierPath = path.join(__dirname, "stillwind-carrier.css");
const relativeRequest = (from, to) => {
  const value = path.relative(from, to).replaceAll("\\", "/");
  return value.startsWith(".") ? value : `./${value}`;
};

module.exports = function (source, inputMap, additionalData) {
  const callback = this.async();
  const text = String(source);
  const prior = additionalData?.stillwind?.sourceTransform;
  // Framework loaders can clone the same rule into a subrequest's chain.
  // Skip only an identical output already produced for this resource.
  if (prior?.resource === this.resourcePath && prior.fingerprint === fingerprint(text))
    return callback(null, source, inputMap, additionalData);
  const projection = /[?&]stillwind\.css(?:&|$)/.test(this.resourceQuery ?? "");
  const cssValue = /[?&]stillwind-module-css(?:&|$)/.test(this.resourceQuery ?? "");
  const sfc = /\.(vue|svelte|astro)$/.test(this.resourcePath);
  if (/[?&](?:raw|url)(?:[=&]|$)/.test(this.resourceQuery ?? ""))
    return callback(null, source, inputMap);
  if (
    !projection &&
    !cssValue &&
    !/stillwind\/(?:react|preact|solid|vue|svelte|astro|module-css)/.test(text)
  )
    return callback(null, source, inputMap);
  const context = this;
  const suppliedOptions = this.getOptions();
  const options = suppliedOptions.stillwindOptions
    ? JSON.parse(Buffer.from(suppliedOptions.stillwindOptions, "base64url").toString("utf8"))
    : suppliedOptions;
  if (!options.css) return callback(new Error("Stillwind requires a CSS entrypoint"));
  let facts;
  // Loader requests remain reconstructible in another worker. Their payload
  // contains configuration, not generated CSS or mutable process-local state.
  const virtualRequest = (query) =>
    sfc
      ? `!!${__filename.replaceAll("\\", "/")}?stillwindOptions=${Buffer.from(JSON.stringify(options)).toString("base64url")}!${context.resourcePath.replaceAll("\\", "/")}?${query}`
      : `${relativeRequest(path.dirname(context.resourcePath), context.resourcePath)}?${query}`;
  const read = (id) =>
    new Promise((resolve, reject) =>
      context.fs.readFile(id, (error, value) => (error ? reject(error) : resolve(String(value)))),
    );
  const resolver = (settings) => {
    const resolve = context.getResolve(settings);
    return (specifier, importer) =>
      new Promise((done, reject) =>
        resolve(path.dirname(importer), specifier, (error, id) =>
          error ? reject(error) : done(id),
        ),
      );
  };
  const resolveCss = resolver({
    conditionNames: ["style", "import", "default"],
    extensions: [".css"],
    mainFields: ["style", "main"],
  });
  const resolveJs = resolver({
    dependencyType: "esm",
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
  });
  const resolveAsset = resolver({});
  const watch = (id) => context.addDependency(id);
  watch(__filename);
  void (async () => {
    const host = await import("@stillwind/host");
    const { NativeProjectSession, nativeBindingPath } = await import("@stillwind/native");
    watch(nativeBindingPath);
    watch(require.resolve("@stillwind/native"));
    const hostDirectory = path.dirname(require.resolve("@stillwind/host"));
    for (const file of fs.readdirSync(hostDirectory)) {
      if (file.endsWith(".js")) watch(path.join(hostDirectory, file));
    }
    const root = path.resolve(context.rootContext, options.root ?? ".");
    const css = path.resolve(root, options.css);
    const compilerOptions = { ...options };
    delete compilerOptions.css;
    delete compilerOptions.root;
    const loaded = await host.loadCssGraph(css, compilerOptions, {
      resolve: resolveCss,
      read,
      watch,
    });
    const session = new NativeProjectSession(loaded.loader, false);
    const output = await host.compileModule(
      session,
      () => (facts ??= new host.TypeScriptFactsService(root)),
      projection || cssValue ? await read(context.resourcePath) : text,
      context.resourcePath,
      root,
      {
        resolve: async (specifier, importer) => {
          if (specifier.startsWith("node:") && isBuiltin(specifier))
            return { id: specifier, external: true };
          try {
            const id = await resolveJs(specifier, importer);
            return id ? { id } : null;
          } catch (error) {
            if (isBuiltin(specifier)) return { id: specifier, external: true };
            throw error;
          }
        },
        read,
        watch,
        warn: (message) => context.emitWarning(new Error(message)),
      },
      {
        stylesheetImports:
          sfc && !projection && !cssValue ? [virtualRequest("stillwind.css")] : undefined,
        moduleCssImport: virtualRequest("stillwind-module-css"),
      },
    );
    if (!output?.code)
      return callback(null, projection ? "" : source, projection ? null : inputMap);
    if (output.compositionModules.length > 0) {
      throw new Error(
        "Cross-module style composition requires the verified Vite CSS transport; this loader currently supports same-module composition only.",
      );
    }
    const plan = session.planStylesheet({
      modules: [output.path],
      coverage: { kind: "observed" },
      placement: cssValue ? "document" : "carrier",
    });
    const replacements = new Map();
    const valueResources = [];
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const resolved = await resolveAsset(resource.specifier, path.resolve(root, resource.owner));
      if (!resolved)
        throw new Error(
          `Cannot resolve Stillwind resource ${resource.specifier} from ${resource.owner}`,
        );
      watch(host.cleanModuleId(resolved));
      if (plan.css.includes(resource.placeholder))
        valueResources.push({
          placeholder: resource.placeholder,
          request: relativeRequest(path.dirname(context.resourcePath), resolved),
        });
      replacements.set(
        resource.placeholder,
        JSON.stringify(relativeRequest(path.dirname(carrierPath), resolved)).slice(1, -1),
      );
    }
    const carrier = relativeRequest(path.dirname(context.resourcePath), carrierPath);
    const upstream =
      !projection && inputMap
        ? {
            id: context.resourcePath,
            map: typeof inputMap === "string" ? inputMap : JSON.stringify(inputMap),
          }
        : undefined;
    const cssImport = (output) =>
      `import ${JSON.stringify(carrier + "?stillwind=" + Buffer.from(host.cssWithSourceMap(output, replacements, root, upstream, !projection)).toString("base64url"))};\n`;
    const base = session.baseCssWithMap();
    if (cssValue) {
      // This JavaScript module owns the literal; the original SFC only receives
      // an import. Asset imports let the caller's bundler choose final URLs.
      const imports = valueResources
        .map((resource, index) => `import asset${index} from ${JSON.stringify(resource.request)};`)
        .join("\n");
      const css = host.cssWithSourceMap(plan, new Map(), root, upstream);
      const replace = valueResources
        .map(
          (resource, index) =>
            `.replaceAll(${JSON.stringify(resource.placeholder)}, JSON.stringify(typeof asset${index} === 'string' ? asset${index} : asset${index}.src).slice(1,-1))`,
        )
        .join("");
      return callback(null, `${imports}\nexport default ${JSON.stringify(css)}${replace};`, null);
    }
    if (projection) return callback(null, cssImport(base) + cssImport(plan));
    const finalized = host.finalizeReactModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      sfc || output.hasModuleCssImport
        ? finalized.code
        : finalized.code.slice(finalized.prefix.length) +
          "\n" +
          finalized.prefix +
          cssImport(base) +
          cssImport(plan);
    const map = host.moduleSourceMap(
      text,
      output.code,
      context.resourcePath,
      output.mappings,
      finalized.removed,
      "",
    );
    const composedMap = host.composeModuleSourceMap(map, inputMap);
    if (sfc && context.data)
      context.data.stillwindSourceTransform = {
        code,
        map: composedMap,
        resource: context.resourcePath,
      };
    callback(null, code, composedMap, {
      ...additionalData,
      stillwind: {
        coverage: plan.coverage,
        designSystem: plan.designSystem,
        producers: plan.producers,
        sourceTransform: { resource: context.resourcePath, fingerprint: fingerprint(code) },
      },
    });
  })()
    .catch((error) => callback(error))
    .finally(() => facts?.close());
};

// Allocate loader-chain-local data for the post-framework map bridge.
module.exports.pitch = function () {};
