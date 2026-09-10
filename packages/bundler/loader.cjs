const path = require("node:path");
const fs = require("node:fs");
const { isBuiltin } = require("node:module");

const carrierPath = path.join(__dirname, "stillwind-carrier.css");
const relativeRequest = (from, to) => {
  const value = path.relative(from, to).replaceAll("\\", "/");
  return value.startsWith(".") ? value : `./${value}`;
};

module.exports = function (source, inputMap) {
  const callback = this.async();
  const text = String(source);
  const projection = /[?&]stillwind\.css(?:&|$)/.test(this.resourceQuery ?? "");
  if (!projection && !text.includes("stillwind/react") && !text.includes("stillwind/solid"))
    return callback(null, source, inputMap);
  const context = this;
  const options = this.getOptions();
  if (!options.css) return callback(new Error("Stillwind requires a CSS entrypoint"));
  let facts;
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
    const compilerOptions = { classIdentity: "module", ...options };
    delete compilerOptions.css;
    delete compilerOptions.root;
    const loaded = await host.loadCssGraph(css, JSON.stringify(compilerOptions), {
      resolve: resolveCss,
      read,
      watch,
    });
    const session = new NativeProjectSession(loaded.loader, false);
    const output = await host.compileModule(
      session,
      () => (facts ??= new host.TypeScriptFactsService(root)),
      projection ? await read(context.resourcePath) : text,
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
    );
    if (!output?.code)
      return callback(null, projection ? "" : source, projection ? null : inputMap);
    const plan = JSON.parse(
      session.planStylesheet(
        JSON.stringify({
          modules: [output.path],
          coverage: { kind: "observed" },
          placement: "carrier",
        }),
      ),
    );
    const replacements = new Map();
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const resolved = await resolveAsset(resource.specifier, path.resolve(root, resource.owner));
      if (!resolved)
        throw new Error(
          `Cannot resolve Stillwind resource ${resource.specifier} from ${resource.owner}`,
        );
      watch(host.cleanModuleId(resolved));
      replacements.set(
        resource.placeholder,
        JSON.stringify(relativeRequest(path.dirname(carrierPath), resolved)).slice(1, -1),
      );
    }
    const carrier = relativeRequest(path.dirname(context.resourcePath), carrierPath);
    const cssImport = (output) =>
      `import ${JSON.stringify(carrier + "?stillwind=" + Buffer.from(host.cssWithSourceMap(output, replacements, root)).toString("base64url"))};\n`;
    const base = JSON.parse(session.baseCssWithMap());
    if (projection) return callback(null, cssImport(base) + cssImport(plan));
    const finalized = host.finalizeReactModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      finalized.code.slice(finalized.prefix.length) +
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
    callback(null, code, host.composeModuleSourceMap(map, inputMap), {
      stillwind: {
        coverage: plan.coverage,
        designSystem: plan.designSystem,
        producers: plan.producers,
      },
    });
  })()
    .catch((error) => callback(error))
    .finally(() => facts?.close());
};
