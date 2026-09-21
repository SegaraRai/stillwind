const path = require("node:path");
const { createHash } = require("node:crypto");

const installed = new WeakSet();

/** Links the completed webpack/Rspack graph before CSS modules are code-generated. */
function registerEmission(context, input) {
  const compilation = context._compilation;
  if (!compilation?.hooks?.finishModules || (input && !context._module)) {
    throw new Error(
      "Automatic Stillwind CSS requires a complete-graph webpack/Rspack integration; use stillwind/module-css for explicit transport in loader-only hosts",
    );
  }
  if (input) context._module.buildInfo.stillwind = input;
  if (installed.has(compilation)) return;
  if (context._compiler?.rspack) {
    throw new Error(
      "Stillwind Rspack emission requires new StillwindPlugin() alongside stillwindRules()",
    );
  }
  installed.add(compilation);
  const filesystem = context.fs;
  const resolveModule = context.getResolve({
    dependencyType: "esm",
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".vue", ".svelte", ".astro"],
  });
  const resolveStylesheet = context.getResolve({
    conditionNames: ["style", "import", "default"],
    extensions: [".css"],
    mainFields: ["style", "main"],
  });
  const resolveResource = context.getResolve({});
  compilation.hooks.finishModules.tapPromise("StillwindEmission", async (modules) => {
    const producers = [...modules].filter((module) => module.buildInfo?.stillwind);
    const carriers = [...modules].filter((module) =>
      module.resource?.includes("stillwind-carrier.css?stillwind-document"),
    );
    if (!carriers.length || !producers.length) return;
    const host = await import("@stillwind/host");
    const { NativeProjectSession } = await import("@stillwind/native");
    const read = (id) =>
      new Promise((resolve, reject) =>
        filesystem.readFile(id, (error, value) => (error ? reject(error) : resolve(String(value)))),
      );
    const resolver = (resolve) => {
      return (specifier, importer) =>
        new Promise((done, reject) =>
          resolve(path.dirname(importer), specifier, (error, id) =>
            error ? reject(error) : done(id),
          ),
        );
    };
    const resolveCss = resolver(resolveStylesheet);
    const resolveJs = resolver(resolveModule);
    const resolveAsset = resolver(resolveResource);
    const { options, root } = producers[0].buildInfo.stillwind;
    const loaded = await host.loadCssGraph(path.resolve(root, options.css), options, {
      read,
      resolve: resolveCss,
      watch: (id) => compilation.fileDependencies.add(id),
    });
    const session = new NativeProjectSession(loaded.loader, false);
    const facts = new host.TypeScriptFactsService(root);
    const selection = [];
    try {
      for (const producer of producers) {
        const input = producer.buildInfo.stillwind;
        if (input.root !== root || JSON.stringify(input.options) !== JSON.stringify(options))
          throw new Error("A compilation must use one Stillwind design system");
        const output = await host.compileModule(
          session,
          () => facts,
          input.source,
          input.id,
          root,
          {
            resolve: async (specifier, importer) => ({ id: await resolveJs(specifier, importer) }),
            read,
            watch: (id) => compilation.fileDependencies.add(id),
            warn: (message) => compilation.warnings.push(new Error(message)),
          },
          { moduleCssImport: "stillwind/module-css" },
        );
        if (output && !input.manual) selection.push(output.path);
      }
      const revision = createHash("sha256")
        .update(producers.map((module) => module.buildInfo.stillwind.source).join("\0"))
        .digest("hex");
      const plan = session.planStylesheet({
        modules: selection,
        coverage: { kind: "complete", target: "compilation", revision },
        placement: "document",
      });
      const replacements = new Map();
      for (const resource of [...loaded.resources, ...plan.resources]) {
        const file = await resolveAsset(resource.specifier, path.resolve(root, resource.owner));
        compilation.fileDependencies.add(file);
        let request = path.relative(__dirname, file).replaceAll("\\", "/");
        if (!request.startsWith(".")) request = `./${request}`;
        replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
      }
      compilation.__stillwindCss = host.cssWithSourceMap(plan, replacements, root);
      for (const carrier of carriers)
        await new Promise((resolve, reject) =>
          compilation.rebuildModule(carrier, (error) => (error ? reject(error) : resolve())),
        );
    } finally {
      facts.close();
    }
  });
}

class StillwindPlugin {
  apply(compiler) {
    compiler.hooks.thisCompilation.tap("StillwindEmission", (compilation) => {
      registerEmission({
        _compilation: compilation,
        fs: compiler.inputFileSystem,
        getResolve(settings) {
          const resolver = compiler.resolverFactory.get("normal", settings);
          return (directory, specifier, callback) =>
            resolver.resolve({}, directory, specifier, {}, callback);
        },
      });
    });
  }
}

module.exports = { registerEmission, StillwindPlugin };
