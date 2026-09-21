import path from "node:path";
import { fileURLToPath } from "node:url";

import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import { compileGraph, createProject, type TurbopackLoaderOptions } from "./project.ts";

export default function (this: LoaderContext<TurbopackLoaderOptions>) {
  emitStylesheet(this, async () => this.getOptions().entries);
}

export function emitStylesheet(
  context: LoaderContext<TurbopackLoaderOptions>,
  entries: () => Promise<string[]>,
): void {
  const callback = context.async();
  void (async () => {
    const project = await createProject(context);
    try {
      const plan = await compileGraph(await entries(), project);
      const replacements = new Map<string, string>();
      for (const resource of [...project.loaded.resources, ...plan.resources]) {
        const file = await project.resolveAsset(
          resource.specifier,
          path.resolve(project.options.root, resource.owner),
        );
        project.host.watch(file);
        let request = path.relative(path.dirname(context.resourcePath), file).replaceAll("\\", "/");
        if (!request.startsWith(".")) {
          request = `./${request}`;
        }
        replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
      }
      const output = decodeCssSourceMap(cssWithSourceMap(plan, replacements, project.options.root));
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
