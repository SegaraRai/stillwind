import { createHash } from "node:crypto";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  cleanModuleId,
  compilerModuleId,
  fileModuleDescriptor,
  loadCssGraph,
} from "@stillwind/host";
import { nativeBindingPath, scanSource } from "@stillwind/native";
import type { LoaderContext } from "webpack";

import {
  compilerOptions,
  cssEntrypoint,
  writeVocabulary,
  type StillwindOptions,
} from "../options.ts";
import { IntegrationSession } from "../session.ts";

export interface TurbopackLoaderOptions {
  compiler: StillwindOptions;
  root: string;
  entries: string[];
  mode: string;
}

export async function createProject(context: LoaderContext<TurbopackLoaderOptions>) {
  const options = context.getOptions();
  const watch = (id: string) => context.addDependency(id);
  watch(nativeBindingPath);
  for (const name of ["@stillwind/host", "@stillwind/native"]) {
    watch(fileURLToPath(import.meta.resolve(name)));
  }
  watch(import.meta.filename);
  for (const file of ["loader.js", "css-loader.js"]) {
    watch(
      fileURLToPath(new URL(`./${file}`, import.meta.resolve("@stillwind/integration/turbopack"))),
    );
  }
  const read = (id: string) =>
    new Promise<string>((done, reject) => {
      context.fs.readFile(id, (error, contents) =>
        error ? reject(error) : done(String(contents)),
      );
    });
  const resolver = (settings: Parameters<typeof context.getResolve>[0]) => {
    const resolve = context.getResolve(settings);
    return (specifier: string, importer: string) =>
      new Promise<string>((done, reject) => {
        resolve(path.dirname(importer), specifier, (error, id) =>
          error
            ? reject(error)
            : id
              ? done(id)
              : reject(new Error(`Cannot resolve ${specifier} from ${importer}`)),
        );
      });
  };
  const resolveJs = resolver({
    extensions: [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json"],
    dependencyType: "esm",
  });
  const resolveAsset = resolver({});
  const loaded = await loadCssGraph(
    path.resolve(options.root, cssEntrypoint(options.compiler)),
    compilerOptions(options.compiler),
    {
      read,
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch,
      resolve: resolver({
        conditionNames: ["style", "import", "default"],
        extensions: [".css"],
        mainFields: ["style", "main"],
      }),
    },
  );
  writeVocabulary(loaded.loader, options.root, options.compiler);
  const compiler = new IntegrationSession(
    options.root,
    loaded.loader,
    options.compiler,
    false,
    options.mode,
  );
  const host = {
    describe: (id: string) => fileModuleDescriptor(options.root, id),
    read,
    watch,
    resolve: async (specifier: string, importer: string) =>
      isBuiltin(specifier)
        ? { id: specifier, external: true }
        : { id: await resolveJs(specifier, importer) },
    warn: (message: string) => context.emitWarning(new Error(message)),
  };
  return { options, compiler, host, loaded, resolveAsset };
}

/** Reconstructs all entry-reachable producers before requesting one document plan. */
export async function compileGraph(
  entries: string[],
  project: Awaited<ReturnType<typeof createProject>>,
) {
  const { options, compiler, host } = project;
  const pending = entries.map((entry) => path.resolve(options.root, entry));
  const visited = new Set<string>();
  const selection: string[] = [];
  const revision = createHash("sha256");
  while (pending.length) {
    const id = pending.shift()!;
    if (visited.has(id)) {
      continue;
    }
    visited.add(id);
    host.watch(id);
    const source = await host.read(id);
    revision.update(id).update("\0").update(source).update("\0");
    const summary = scanSource({
      module_id: compilerModuleId(options.root, id),
      uri: pathToFileURL(id).href,
      source,
      language: /\.tsx(?:[?#]|$)/i.test(id)
        ? "tsx"
        : /\.jsx(?:[?#]|$)/i.test(id)
          ? "jsx"
          : /\.d\.[cm]?ts(?:[?#]|$)/i.test(id)
            ? "dts"
            : /\.[cm]?ts(?:[?#]|$)/i.test(id)
              ? "ts"
              : "js",
      module_kind: "unambiguous",
      source_map_source: pathToFileURL(id).href,
    });
    for (const { source: specifier } of summary.module_specifiers) {
      if (isBuiltin(specifier) || specifier === "stillwind" || specifier.startsWith("stillwind/")) {
        continue;
      }
      const resolved = await host.resolve(specifier, id);
      const file = cleanModuleId(resolved.id);
      if (
        !resolved.external &&
        !/[\\/]node_modules[\\/]/.test(file) &&
        /\.[cm]?[jt]sx?$/.test(file)
      ) {
        pending.push(file);
      }
    }
    if (!summary.macro_imports && !summary.module_css_imports) {
      continue;
    }
    const output = await compiler.compile(source, id, host, {
      moduleCssImport: "stillwind/module-css",
    });
    if (output && !output.hasModuleCssImport) {
      selection.push(output.module_id);
    }
  }
  return compiler.native.planStylesheet({
    modules: selection,
    coverage: { kind: "complete", target: "entry-graph", revision: revision.digest("hex") },
    placement: "document",
  });
}
