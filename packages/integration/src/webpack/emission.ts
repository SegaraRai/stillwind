import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { fileModuleDescriptor } from "@stillwind/host";

import {
  compilerOptions,
  cssEntrypoint,
  writeVocabulary,
  type StillwindOptions,
} from "../options.ts";
import { IntegrationSession } from "../session.ts";

interface ProducerInput {
  source: string;
  id: string;
  root: string;
  options: StillwindOptions;
  manual: boolean;
}

const installed = new WeakSet();

/** Links the completed webpack/Rspack graph before CSS modules are code-generated. */
function registerEmission(context: any, input?: ProducerInput) {
  const compilation = context._compilation;
  if (!compilation?.hooks?.finishModules || (input && !context._module)) {
    throw new Error(
      "Automatic Stillwind CSS requires a complete-graph webpack/Rspack integration; use stillwind/module-css for explicit transport in loader-only hosts",
    );
  }
  if (input) {
    context._module.buildInfo.stillwind = input;
  }
  if (installed.has(compilation)) {
    return;
  }
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
  compilation.hooks.finishModules.tapPromise(
    "StillwindEmission",
    async (modules: Iterable<any>) => {
      const producers = [...modules].filter((module) => module.buildInfo?.stillwind);
      const carriers = [...modules].filter((module) =>
        module.resource?.includes("stillwind-webpack.css?stillwind-document"),
      );
      if (!carriers.length || !producers.length) {
        return;
      }
      const host = await import("@stillwind/host");
      const read = (id: string) =>
        new Promise<string>((resolve, reject) =>
          filesystem.readFile(id, (error: Error | null, value: unknown) =>
            error ? reject(error) : resolve(String(value)),
          ),
        );
      const resolver = (resolve: any) => {
        return (specifier: string, importer: string) =>
          new Promise<string>((done, reject) =>
            resolve(path.dirname(importer), specifier, (error: Error | null, id: string) =>
              error ? reject(error) : done(id),
            ),
          );
      };
      const resolveCss = resolver(resolveStylesheet);
      const resolveJs = resolver(resolveModule);
      const resolveAsset = resolver(resolveResource);
      const { options, root } = producers[0].buildInfo.stillwind;
      const loaded = await host.loadCssGraph(
        path.resolve(root, cssEntrypoint(options)),
        compilerOptions(options),
        {
          read,
          resolve: resolveCss,
          documentUri: (id) => pathToFileURL(id).href,
          sourceMapSource: (id) => pathToFileURL(id).href,
          watch: (id) => compilation.fileDependencies.add(id),
        },
      );
      writeVocabulary(loaded.loader, root, options);
      const compiler = new IntegrationSession(
        root,
        loaded.loader,
        options,
        false,
        context.mode === "development" ? "development" : "release",
      );
      const session = compiler.native;
      const selection = [];
      try {
        for (const producer of producers) {
          const input = producer.buildInfo.stillwind;
          if (input.root !== root || JSON.stringify(input.options) !== JSON.stringify(options)) {
            throw new Error("A compilation must use one Stillwind design system");
          }
          const output = await compiler.compile(
            input.source,
            input.id,
            {
              describe: (id) => fileModuleDescriptor(root, id),
              resolve: async (specifier, importer) => ({
                id: await resolveJs(specifier, importer),
              }),
              read,
              watch: (id) => compilation.fileDependencies.add(id),
              warn: (message) => compilation.warnings.push(new Error(message)),
            },
            { moduleCssImport: "stillwind/module-css" },
          );
          if (output && !input.manual) {
            selection.push(output.module_id);
          }
        }
        const revision = createHash("sha256")
          .update(producers.map((module) => `${module.buildInfo.stillwind.source}\0`).join(""))
          .digest("hex");
        const plan = session.planStylesheet({
          modules: selection,
          coverage: { kind: "complete", target: "compilation", revision },
          placement: "document",
        });
        const replacements = new Map();
        for (const resource of [...loaded.resources, ...plan.resources]) {
          const owner =
            "module_id" in resource
              ? compiler.resolverId(resource.module_id)
              : resource.source_name;
          const file = await resolveAsset(resource.specifier, owner);
          compilation.fileDependencies.add(file);
          let request = path
            .relative(
              path.dirname(fileURLToPath(import.meta.resolve("@stillwind/integration/webpack"))),
              file,
            )
            .replaceAll("\\", "/");
          if (!request.startsWith(".")) {
            request = `./${request}`;
          }
          replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
        }
        compilation.__stillwindCss = host.cssWithSourceMap(plan, replacements);
        for (const carrier of carriers) {
          await new Promise<void>((resolve, reject) =>
            compilation.rebuildModule(carrier, (error: Error | null) =>
              error ? reject(error) : resolve(),
            ),
          );
        }
      } finally {
        compiler.close();
      }
    },
  );
}

class StillwindPlugin {
  apply(compiler: any) {
    compiler.hooks.thisCompilation.tap("StillwindEmission", (compilation: any) => {
      registerEmission({
        _compilation: compilation,
        fs: compiler.inputFileSystem,
        mode: compiler.options.mode,
        getResolve(settings: any) {
          const resolver = compiler.resolverFactory.get("normal", settings);
          return (directory: string, specifier: string, callback: any) =>
            resolver.resolve({}, directory, specifier, {}, callback);
        },
      });
    });
  }
}

export { registerEmission, StillwindPlugin };
