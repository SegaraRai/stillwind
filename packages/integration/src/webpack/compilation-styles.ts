import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  cleanModuleId,
  cssEntrypoint,
  cssWithSourceMap,
  loadCssGraph,
  nodePluginHost,
  warnRejectedNames,
  type CssUpstreamMap,
} from "@stillwind/host";

import { buildCompilerOptions, IntegrationSession } from "../session.ts";
import type { ProducerInput } from "./emission.ts";
import type { CompilationHost } from "./host.ts";
import { relativeRequest } from "./requests.ts";
import { sourceDescriptor } from "./sources.ts";

/**
 * Recompiles recorded producers and renders document CSS for a completed compilation.
 *
 * Manual producers are compiled for dependency composition but are excluded from the document
 * selection. The temporary compiler closes after reconstruction.
 *
 * @param producers - Nonempty list of producers from the completed graph. Every producer must use
 * the same root and config.
 * @param io - Compilation host that reads captured dependency sources and resolves CSS, scripts,
 * and assets.
 * @param mode - Bundler mode. Only `development` selects development compilation.
 * @param watch - Registers a file as a compilation dependency.
 * @param warn - Reports a compiler warning through the bundler.
 * @returns Document CSS with asset requests and an inline source map composed with recorded
 * upstream maps.
 * @throws If producers use different design systems, a dependency source is missing, or loading or
 * compilation fails.
 */
export async function compilationStyles(
  producers: readonly ProducerInput[],
  io: CompilationHost,
  mode: string | undefined,
  watch: (id: string) => void,
  warn: (message: string) => void,
): Promise<string> {
  const { config, root } = producers[0];
  const readDependency = io.dependencyReader(producers);
  const loaded = await loadCssGraph(
    path.resolve(root, cssEntrypoint(config)),
    buildCompilerOptions(config),
    {
      read: io.read,
      resolve: io.resolveCss,
      documentUri: (id) => pathToFileURL(id).href,
      sourceMapSource: (id) => pathToFileURL(id).href,
      watch,
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
  const compiler = new IntegrationSession(
    root,
    loaded.loader,
    config,
    false,
    mode === "development" ? "development" : "release",
    loaded.plugins,
  );
  const selection: string[] = [];
  try {
    for (const input of producers) {
      if (input.root !== root || JSON.stringify(input.config) !== JSON.stringify(config)) {
        throw new Error("A compilation must use one Stillwind design system");
      }
      const output = await compiler.compile(
        input.source,
        input.id,
        {
          describe: (id) => sourceDescriptor(root, id),
          resolve: async (specifier, importer) => ({
            id: await io.resolveJs(specifier, importer),
          }),
          read: readDependency,
          watch: (id) => {
            watch(cleanModuleId(id));
          },
          warn,
        },
        {
          moduleCssImport: "stillwind/module-css",
        },
      );
      if (output && !input.manual) {
        selection.push(output.moduleId);
      }
    }
    const revision = createHash("sha256")
      .update(producers.map((input) => `${input.source}\0`).join(""))
      .digest("hex");
    const plan = compiler.native.planStylesheet({
      modules: selection,
      coverage: {
        kind: "complete",
        target: "compilation",
        revision,
      },
      placement: "document",
    });
    warnRejectedNames(plan.rejectedNames, warn);
    const replacements = new Map<string, string>();
    for (const resource of [...loaded.resources, ...plan.resources]) {
      const owner =
        "moduleId" in resource ? compiler.resolverId(resource.moduleId) : resource.sourceName;
      const file = await io.resolveAsset(resource.specifier, owner);
      watch(file);
      const request = relativeRequest(
        path.dirname(fileURLToPath(import.meta.resolve("@stillwind/integration/webpack"))),
        file,
      );
      replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
    }
    return cssWithSourceMap(
      plan,
      replacements,
      producers.flatMap((input) =>
        [
          input.upstream,
          ...(input.dependencies ?? []).map((dependency) => dependency.upstream),
        ].filter((upstream): upstream is CssUpstreamMap => Boolean(upstream)),
      ),
    );
  } finally {
    compiler.close();
  }
}
