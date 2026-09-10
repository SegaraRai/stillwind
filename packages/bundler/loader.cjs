const path = require("node:path");
const fs = require("node:fs");

const carrierPath = path.join(__dirname, "stillwind-carrier.css");
const relativeRequest = (from, to) => {
  const value = path.relative(from, to).replaceAll("\\", "/");
  return value.startsWith(".") ? value : `./${value}`;
};

module.exports = function (source, inputMap) {
  const callback = this.async();
  const text = String(source);
  if (!text.includes("stillwind/react") && !text.includes("stillwind/solid"))
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
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
  });
  const resolveAsset = resolver({});
  const watch = (id) => context.addDependency(id);
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
      text,
      context.resourcePath,
      root,
      {
        resolve: async (specifier, importer) => {
          const id = await resolveJs(specifier, importer);
          return id ? { id } : null;
        },
        read,
        watch,
        warn: (message) => context.emitWarning(new Error(message)),
      },
    );
    if (!output?.code) return callback(null, source, inputMap);
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
    const resolveResources = (css) => {
      for (const [placeholder, url] of replacements) css = css.replaceAll(placeholder, url);
      return css;
    };
    const carrier = relativeRequest(path.dirname(context.resourcePath), carrierPath);
    const cssImport = (css) =>
      `import ${JSON.stringify(carrier + "?stillwind=" + Buffer.from(resolveResources(css)).toString("base64url"))};\n`;
    const finalized = host.finalizeReactModuleWithMap(output.code);
    // Append synthetic imports so RSC directives remain the first statements.
    const code =
      finalized.code.slice(finalized.prefix.length) +
      "\n" +
      finalized.prefix +
      cssImport(session.baseCss()) +
      cssImport(plan.css);
    const map = host.moduleSourceMap(
      text,
      output.code,
      context.resourcePath,
      output.mappings,
      finalized.removed,
      "",
    );
    callback(null, code, map, {
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
