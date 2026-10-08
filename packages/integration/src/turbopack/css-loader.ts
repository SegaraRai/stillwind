import path from "node:path";
import { fileURLToPath } from "node:url";

import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import { loaderCompletion } from "../webpack/source-map-loader.ts";
import { compileGraph, createProject, type TurbopackLoaderOptions } from "./project.ts";

export default function turbopackCssLoader(this: LoaderContext<TurbopackLoaderOptions>) {
  emitStylesheet(this, async (options) => options.entries);
}

export function emitStylesheet(
  context: LoaderContext<TurbopackLoaderOptions>,
  entries: (options: TurbopackLoaderOptions) => Promise<string[]>,
): void {
  const callback = loaderCompletion(context);
  void (async () => {
    const project = await createProject(context);
    try {
      const plan = await compileGraph(await entries(project.options), project);
      const replacements = new Map<string, string>();
      for (const resource of [...project.loaded.resources, ...plan.resources]) {
        const file = await project.resolveAsset(
          resource.specifier,
          "moduleId" in resource
            ? project.compiler.resolverId(resource.moduleId)
            : resource.sourceName,
        );
        project.host.watch(file);
        let request = path.relative(path.dirname(context.resourcePath), file).replaceAll("\\", "/");
        if (!request.startsWith(".")) {
          request = `./${request}`;
        }
        replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
      }
      const output = decodeCssSourceMap(cssWithSourceMap(plan, replacements));
      if (output.map) {
        output.map.sources = output.map.sources.map((source: string) =>
          source.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
        );
      }
      callback(null, output.code, output.map);
    } finally {
      project.compiler.close();
    }
  })().catch(callback);
}
