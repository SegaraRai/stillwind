import { posix } from "node:path";

import type { Rollup } from "vite";

import { producerPrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

/**
 * Associates emitted CSS with a chunk and adds ordered stylesheet links for split emission.
 *
 * Split parts retain manifest order and relative-base URLs resolve against the chunk. Single
 * emission adds its asset to Vite CSS metadata without changing the JavaScript.
 *
 * @param project - Project with build output produced by `renderStart`.
 * @param name - Vite environment name, or `undefined` for `client`.
 * @param context - Build context that reads module dependencies and resolves emitted filenames.
 * @param code - Rendered JavaScript for the chunk.
 * @param chunk - Chunk whose modules select CSS parts and whose Vite metadata is updated.
 * @returns JavaScript with a split stylesheet loader and a null map when parts are selected,
 * otherwise `null`.
 */
export function renderChunkStyles(
  project: ViteProject,
  name: string | undefined,
  context: Pick<Rollup.PluginContext, "getModuleInfo" | "getFileName">,
  code: string,
  chunk: Rollup.RenderedChunk,
) {
  const { config, settings, inputs, moduleRegistry } = project;
  const { manifest, buildCss } = project.buildOutput(name);

  /**
   * Records a stylesheet in the chunk's Vite metadata, creating the metadata when absent.
   * @param file - Emitted asset filename without the public base prefix.
   */
  const attachCss = (file: string) => {
    chunk.viteMetadata ??= {
      importedCss: new Set(),
      importedAssets: new Set(),
      __modules: new Set(),
    };
    chunk.viteMetadata.importedCss.add(file);
  };

  if (config.emission === "split" && manifest) {
    const graph = new Set(Object.keys(chunk.modules));
    if (chunk.facadeModuleId) {
      graph.add(chunk.facadeModuleId);
    }
    for (const id of graph) {
      for (const dependency of context.getModuleInfo(id)?.importedIds ?? []) {
        graph.add(dependency);
      }
    }
    const ids = new Set(
      [...graph].flatMap((id) => {
        const moduleId =
          inputs(name).get(id)?.moduleId ??
          context.getModuleInfo(id)?.meta.stillwind?.moduleId ??
          (id.startsWith(producerPrefix)
            ? (() => {
                const entry = moduleRegistry(name).get(id);
                return entry?.kind === "producer" ? entry.moduleId : "";
              })()
            : "");
        return manifest.modules[moduleId] ?? [];
      }),
    );
    const parts = manifest.parts
      .filter((part) => ids.has(part.id))
      .map((part) => ({
        ...part,
        order: manifest.parts.indexOf(part),
        relative: settings.publicBase === "./" || settings.publicBase === "",
        href:
          settings.publicBase === "./" || settings.publicBase === ""
            ? posix.relative(
                posix.dirname(chunk.fileName),
                part.href.slice(settings.publicBase.length),
              )
            : part.href,
      }));
    if (parts.length) {
      for (const part of manifest.parts.filter((part) => ids.has(part.id))) {
        attachCss(part.href.slice(settings.publicBase.length));
      }
      return {
        code: `${code}

/* Stillwind CSS Loader */
if (typeof document !== "undefined") {
  for (const p of ${JSON.stringify(parts)}) {
    const links = [...document.querySelectorAll("link[data-stillwind-part]")];
    if (links.some((l) => l.dataset.stillwindPart === p.id)) {
      continue;
    }
    const l = document.createElement("link");
    l.rel = "stylesheet";
    l.href = p.relative ? new URL(p.href, import.meta.url).href : p.href;
    l.dataset.stillwindPart = p.id;
    l.dataset.stillwindOrder = String(p.order);
    document.head.insertBefore(l, links.find((e) => Number(e.dataset.stillwindOrder) > p.order) ?? null);
  }
}
`,
        map: null,
      };
    }
  }
  if (buildCss) {
    attachCss(context.getFileName(buildCss));
  }

  return null;
}
