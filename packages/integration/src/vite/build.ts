import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { posix } from "node:path";

import { cssWithSourceMap, decodeCssSourceMap } from "@stillwind/host";
import { transform as transformCss } from "esbuild";
import type { Plugin } from "vite";

import { partitionStylesheet } from "./emission.ts";
import { runtimeId, projectionPrefix, producerPrefix } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";
import { resolveResources } from "./resources.ts";

export function buildHooks(project: ViteProject): Pick<Plugin, "renderStart" | "renderChunk"> {
  const {
    options,
    settings,
    environmentState,
    inputMap,
    includeSourceContent,
    documentPlan,
    compile,
  } = project;
  return {
    async renderStart() {
      if (settings.watching) {
        return;
      }
      const name = this.environment?.name;
      const state = environmentState(name);
      // The final graph includes lazy modules and cached transforms. Recover inputs
      // from graph metadata before making a complete-coverage claim.
      for (const id of this.getModuleIds()) {
        const metadata = this.getModuleInfo(id)?.meta.stillwind;
        if (metadata?.input !== undefined && !inputMap(name).has(metadata.id ?? id)) {
          await compile(name, metadata.input, metadata.id ?? id, this);
        }
        if (metadata?.projection) {
          const input = inputMap(name).get(metadata.id ?? id);
          if (input) {
            input.projection = true;
          }
        }
      }
      const plan = await documentPlan(name, this, true);
      if (
        ![...this.getModuleIds()].some(
          (id) =>
            id === runtimeId || id.startsWith(producerPrefix) || id.startsWith(projectionPrefix),
        )
      ) {
        return;
      }
      const assets = new Map<string, string>();
      let css = plan.css;
      for (const match of css.matchAll(/\/@fs\/([^"'\s)]+)/g)) {
        const file = match[1];
        if (assets.has(file)) {
          continue;
        }
        const reference = this.emitFile({
          type: "asset",
          name: decodeURIComponent(file).split("/").at(-1),
          source: await readFile(decodeURIComponent(file)),
        });
        assets.set(file, reference);
      }
      const resourceUrls = (text: string) => {
        for (const [file, reference] of assets) {
          text = text.replaceAll(
            `/@fs/${file}`,
            posix.relative(settings.assetsDirectory, this.getFileName(reference)),
          );
        }
        return text;
      };
      css = resourceUrls(css);
      const emitCss = async (text: string, sourceMap?: string) => {
        if (settings.minifyCss) {
          const input = sourceMap
            ? `${text}\n/*# sourceMappingURL=data:application/json;base64,${Buffer.from(sourceMap).toString("base64")} */`
            : text;
          const result = await transformCss(input, {
            loader: "css",
            minify: true,
            sourcemap: sourceMap ? "external" : false,
            sourcefile: "stillwind.css",
          });
          text = result.code;
          sourceMap = result.map || undefined;
        }
        const hash = createHash("sha256").update(text).digest("hex").slice(0, 16);
        if (sourceMap) {
          this.emitFile({
            type: "asset",
            fileName: `${settings.assetsDirectory}/stillwind-${hash}.css.map`,
            source: sourceMap,
          });
          text += `\n/*# sourceMappingURL=stillwind-${hash}.css.map */`;
        }
        return this.emitFile({
          type: "asset",
          fileName: `${settings.assetsDirectory}/stillwind-${hash}.css`,
          source: text,
        });
      };
      const href = (reference: string) => settings.publicBase + this.getFileName(reference);
      const revision = createHash("sha256")
        .update(plan.designSystem)
        .update("\0")
        .update(plan.originalCss)
        .update("\0")
        .update(JSON.stringify(plan.producers))
        .update("\0")
        .digest("hex");
      if (options.emission !== "split") {
        const replacements = new Map(
          [...plan.resourceReplacements].map(([key, value]) => [key, resourceUrls(value)]),
        );
        const mapped = settings.sourceMaps
          ? decodeCssSourceMap(
              cssWithSourceMap(
                { css: plan.originalCss, sourceMap: plan.sourceMap },
                replacements,
                [...inputMap(name).values()].flatMap((input) =>
                  input.upstream ? [input.upstream] : [],
                ),
                includeSourceContent(name),
              ),
            )
          : undefined;
        state.buildCss = await emitCss(css, mapped?.map ? JSON.stringify(mapped.map) : undefined);
        state.manifest = {
          version: 1,
          revision,
          parts: [{ id: "document", href: href(state.buildCss) }],
          modules: Object.fromEntries(
            plan.producers.map((producer) => [producer.module, ["document"]]),
          ),
        };
      } else {
        const references = new Map<string, string>();
        const parts: { id: string; href: string }[] = [];
        const partitions = partitionStylesheet(plan.rawParts);
        for (const [index, part] of partitions.entries()) {
          let reference = references.get(part.key);
          if (!reference) {
            const replacements = new Map(
              [...plan.resourceReplacements].map(([key, value]) => [key, resourceUrls(value)]),
            );
            const mapped = settings.sourceMaps
              ? decodeCssSourceMap(
                  cssWithSourceMap(
                    part,
                    replacements,
                    [...inputMap(name).values()].flatMap((input) =>
                      input.upstream ? [input.upstream] : [],
                    ),
                    includeSourceContent(name),
                  ),
                )
              : undefined;
            references.set(
              part.key,
              (reference = await emitCss(
                mapped?.code ?? resolveResources(part.css, replacements),
                mapped?.map ? JSON.stringify(mapped.map) : undefined,
              )),
            );
          }
          parts.push({ id: `${index}:${part.key}`, href: href(reference) });
        }
        const dependencies = new Map(
          [...inputMap(name).values()].map((input) => [input.moduleId, input.dependencies]),
        );
        const modules = Object.fromEntries(
          plan.producers.map((producer) => {
            const closure = new Set([producer.module]);
            for (const module of closure) {
              for (const dependency of dependencies.get(module) ?? []) {
                closure.add(dependency);
              }
            }
            return [
              producer.module,
              parts
                .filter(
                  (_, index) =>
                    partitions[index].producers.length === 0 ||
                    partitions[index].producers.some((owner) => closure.has(owner)),
                )
                .map((part) => part.id),
            ];
          }),
        );
        state.manifest = { version: 1, revision, parts, modules };
      }
      this.emitFile({
        type: "asset",
        fileName: "stillwind-manifest.json",
        source: JSON.stringify(state.manifest, null, 2),
      });
    },
    renderChunk(code, chunk) {
      const { manifest, buildCss } = environmentState(this.environment?.name);
      const metadata = chunk as typeof chunk & {
        viteMetadata?: {
          importedCss: Set<string>;
          importedAssets: Set<string>;
        };
      };
      const attachCss = (file: string) => {
        metadata.viteMetadata ??= {
          importedCss: new Set(),
          importedAssets: new Set(),
          __modules: new Set(),
        };
        metadata.viteMetadata.importedCss.add(file);
      };
      if (options.emission === "split" && manifest) {
        const graph = new Set(Object.keys(chunk.modules));
        if (chunk.facadeModuleId) {
          graph.add(chunk.facadeModuleId);
        }
        for (const id of graph) {
          for (const dependency of this.getModuleInfo(id)?.importedIds ?? []) {
            graph.add(dependency);
          }
        }
        const ids = new Set(
          [...graph].flatMap(
            (id) =>
              manifest!.modules[
                inputMap(this.environment?.name).get(id)?.moduleId ??
                  this.getModuleInfo(id)?.meta.stillwind?.moduleId ??
                  (id.startsWith(producerPrefix)
                    ? Buffer.from(id.slice(producerPrefix.length), "base64url").toString("utf8")
                    : "")
              ] ?? [],
          ),
        );
        const parts = manifest.parts
          .filter((part) => ids.has(part.id))
          .map((part) => ({
            ...part,
            order: manifest!.parts.indexOf(part),
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
        attachCss(this.getFileName(buildCss));
      }
      return null;
    },
  };
}
