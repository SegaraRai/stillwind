import { posix } from "node:path";

import type { Rollup } from "vite";

import { producerPrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";

/**
 * Browser code that keeps loader links across Astro ClientRouter navigations.
 *
 * Astro removes each head element that the incoming document does not contain, and the chunk that
 * added a link does not run again, so the link would stay missing. Astro keeps an old stylesheet
 * link when the incoming head has a link with the same `href`, and each incoming link matches one
 * old link. The listener therefore copies each loader link while the incoming head has fewer links
 * with its `href` than the current head, which also keeps repeated split occurrences. One listener
 * serves all chunks in a document.
 */
const persistAcrossSwaps = `const key = Symbol.for("stillwind.persistLinks");
  if (!globalThis[key]) {
    globalThis[key] = true;
    document.addEventListener("astro:before-swap", (event) => {
      const head = event.newDocument.head;
      const count = (parent, href) =>
        [...parent.querySelectorAll('link[rel="stylesheet"]')].filter((l) => l.getAttribute("href") === href).length;
      for (const link of document.head.querySelectorAll("link[data-stillwind-loader]")) {
        const href = link.getAttribute("href");
        if (count(head, href) < count(document.head, href)) {
          head.append(link.cloneNode());
        }
      }
    });
  }`;

/**
 * Associates emitted CSS with a chunk and loads its stylesheets in the browser.
 *
 * Split parts retain manifest order and relative-base URLs resolve against the chunk. Hosts can
 * request a loader for single emission when they do not deliver CSS from chunk metadata.
 *
 * @param project - Project with build output produced by `renderStart`.
 * @param name - Vite environment name, or `undefined` for `client`.
 * @param context - Build context that reads module dependencies and resolves emitted filenames.
 * @param code - Rendered JavaScript for the chunk.
 * @param chunk - Chunk whose modules select CSS parts and whose Vite metadata is updated.
 * @param loadSingleStylesheet - Loads single emission through a browser link. Defaults to false.
 * @returns JavaScript with a null map when a loader is needed, otherwise `null`.
 */
export function renderChunkStyles(
  project: ViteProject,
  name: string | undefined,
  context: Pick<Rollup.PluginContext, "getModuleInfo" | "getFileName">,
  code: string,
  chunk: Rollup.RenderedChunk,
  loadSingleStylesheet = false,
) {
  const { config, settings, inputs, moduleRegistry } = project;
  const { manifest, buildCss } = project.buildOutput(name);

  /**
   * Records a stylesheet in the chunk's Vite metadata, creating the metadata when absent.
   *
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
    l.dataset.stillwindLoader = "";
    document.head.insertBefore(l, links.find((e) => Number(e.dataset.stillwindOrder) > p.order) ?? null);
  }
  ${persistAcrossSwaps}
}
`,
        map: null,
      };
    }
  }
  if (buildCss) {
    const file = context.getFileName(buildCss);
    attachCss(file);
    if (!loadSingleStylesheet) {
      return null;
    }
    const relative = settings.publicBase === "./" || settings.publicBase === "";
    const href = relative
      ? posix.relative(posix.dirname(chunk.fileName), file)
      : settings.publicBase + file;
    return {
      code: `${code}

/* Stillwind CSS Loader */
if (typeof document !== "undefined") {
  const href = new URL(${JSON.stringify(href)}, ${relative ? "import.meta.url" : "document.baseURI"}).href;
  if (![...document.querySelectorAll('link[rel="stylesheet"]')].some((link) => link.href === href)) {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    link.dataset.stillwindLoader = "";
    document.head.append(link);
  }
  ${persistAcrossSwaps}
}
`,
      map: null,
    };
  }

  return null;
}
