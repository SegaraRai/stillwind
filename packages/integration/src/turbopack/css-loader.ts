import path from "node:path";
import { fileURLToPath } from "node:url";

import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import type { LoaderContext } from "webpack";

import { loaderCompletion } from "../webpack/source-map-loader.ts";
import {
  compileGraph,
  createProject,
  type TurbopackLoaderOptions,
  type TurbopackProject,
} from "./project.ts";

/**
 * Starts document CSS reconstruction from entries captured in standalone Turbopack loader options.
 *
 * @param this - Loader context that receives asynchronous CSS and source-map completion or an error.
 */
export default function turbopackCssLoader(this: LoaderContext<TurbopackLoaderOptions>) {
  emitStylesheet(this, async (options) => options.entries);
}

/**
 * Reconstructs a document stylesheet and resolves its local resources through the bundler.
 *
 * @param context - Loader context whose callback receives CSS and a decoded map, or the operation error.
 * @param entries - Callback that selects absolute or project-relative entries after configuration is evaluated.
 * Repeats reconstruction if entry discovery changes during compilation or resource resolution.
 * Dependency watches are registered during traversal, and each attempt releases its project.
 */
export function emitStylesheet(
  context: LoaderContext<TurbopackLoaderOptions>,
  entries: (options: TurbopackProject["options"]) => Promise<string[]>,
): void {
  const callback = loaderCompletion(context);
  void (async () => {
    for (;;) {
      const project = await createProject(context);
      try {
        const selected = await entries(project.options);
        let plan;
        try {
          plan = await compileGraph(selected, project);
        } catch (error) {
          if (
            (error as NodeJS.ErrnoException).code === "ENOENT" &&
            !sameEntries(selected, await entries(project.options))
          ) {
            continue;
          }
          throw error;
        }
        const replacements = new Map<string, string>();
        for (const resource of [...project.loaded.resources, ...plan.resources]) {
          const file = await project.resolveAsset(
            resource.specifier,
            "moduleId" in resource
              ? project.graphCompiler.resolverId(resource.moduleId)
              : resource.sourceName,
          );
          project.host.watch(file);
          let request = path
            .relative(path.dirname(context.resourcePath), file)
            .replaceAll("\\", "/");
          if (!request.startsWith(".")) {
            request = `./${request}`;
          }
          replacements.set(resource.placeholder, JSON.stringify(request).slice(1, -1));
        }
        // Next runs this loader for the stylesheet carrier of each compiler and layer after one
        // edit. Rendering the stylesheet and its map takes most of a call that compiles nothing.
        const key = JSON.stringify([
          plan.coverage.kind === "complete" ? plan.coverage.revision : "",
          context.resourcePath,
          [...replacements],
        ]);
        const previous = renderedStylesheets.get(project.graphCompiler);
        const output =
          previous?.key === key
            ? previous.output
            : decodeCssSourceMap(cssWithSourceMap(plan, replacements));
        if (output.map && previous?.output !== output) {
          output.map.sources = output.map.sources.map((source: string) =>
            source.startsWith("file:") ? fileURLToPath(source).replaceAll("\\", "/") : source,
          );
        }
        renderedStylesheets.set(project.graphCompiler, { key, output });
        // Turbopack registers context dependencies after a loader finishes. A route added
        // during reconstruction can become part of that baseline without being in this CSS.
        if (!sameEntries(selected, await entries(project.options))) {
          continue;
        }
        // The bundler may change the map that it receives.
        callback(
          null,
          output.code,
          output.map && {
            ...output.map,
            sources: [...output.map.sources],
            sourcesContent: output.map.sourcesContent && [...output.map.sourcesContent],
          },
        );
        return;
      } finally {
        project.release();
      }
    }
  })().catch(callback);
}

/**
 * Last stylesheet that each graph compiler rendered, with a key made of the plan revision, the
 * carrier path, and the resource replacements. Equal keys give equal output.
 */
const renderedStylesheets = new WeakMap<
  TurbopackProject["graphCompiler"],
  {
    key: string;
    output: ReturnType<typeof decodeCssSourceMap>;
  }
>();

function sameEntries(previous: readonly string[], current: readonly string[]): boolean {
  return (
    previous.length === current.length && previous.every((entry, index) => entry === current[index])
  );
}
