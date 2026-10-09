import { createHash } from "node:crypto";

import { decodeCssSourceMap } from "@stillwind/host";
import type { Plugin } from "vite";

import { emitBuildCss, emitBuildResources } from "./build-assets.ts";
import { renderChunkStyles } from "./chunk-styles.ts";
import { partitionStylesheet } from "./emission.ts";
import { producerPrefix, projectionPrefix, runtimeId } from "./module-ids.ts";
import type { ViteProject } from "./project.ts";
import { resolveResources } from "./resources.ts";

/**
 * A production stylesheet manifest that preserves CSS part order and records each producer's
 * required parts.
 */
export interface StylesheetManifest {
  /** Serialized manifest schema version. Consumers must interpret this structure as version 1. */
  version: 1;
  /** SHA-256 fingerprint of the design system, original CSS, and producer artifacts. */
  revision: string;
  /**
   * Stylesheet occurrences in cascade order. Repeated payloads may share a URL but retain distinct
   * IDs.
   */
  parts: {
    /** Occurrence identity referenced by `modules`. Single emission uses `document`. */
    id: string;
    /** Public URL formed from the configured Vite base and emitted asset filename. */
    href: string;
  }[];
  /**
   * Compiler module identities mapped to required part IDs in manifest order, including shared and
   * composed styles.
   */
  modules: Record<string, string[]>;
}

/** Emitted CSS references and stylesheet metadata for one Vite environment. */
export interface BuildOutput {
  /**
   * Bundler reference ID for the single document CSS asset. Split emission leaves this undefined.
   */
  buildCss?: string;
  /** Manifest for emitted document styles. Undefined until a build has emitted Stillwind CSS. */
  manifest?: StylesheetManifest;
}

/**
 * Creates production hooks that emit complete document CSS and associate it with rendered chunks.
 *
 * @param project - Project whose compiler hooks collect producers and initialize environment
 * settings.
 * @returns Hooks for stylesheet emission at render start and CSS association during chunk
 * rendering.
 */
export function buildHooks(project: ViteProject): Pick<Plugin, "renderStart" | "renderChunk"> {
  const { config, settings, inputs, cssForBrowser, documentPlan, compile } = project;

  return {
    /**
     * Recovers cached producers from the completed graph and emits CSS, resources, and the
     * manifest.
     *
     * The Vite hook context must expose the completed module graph. Recovered inputs include lazy
     * modules before the stylesheet receives complete coverage.
     *
     * @returns A promise that resolves after build output is stored, or immediately for development
     * and graphs without document CSS references.
     */
    async renderStart() {
      if (settings.watching) {
        return;
      }
      const name = this.environment?.name;
      // The final graph includes lazy modules and cached transforms. Recover inputs
      // from graph metadata before making a complete-coverage claim.
      for (const id of this.getModuleIds()) {
        const metadata = this.getModuleInfo(id)?.meta.stillwind;
        if (metadata?.input !== undefined && !inputs(name).has(metadata.id ?? id)) {
          await compile(name, metadata.input, metadata.id ?? id, this);
        }
        if (metadata?.projection) {
          await project.environment(name).markProjection(metadata.id ?? id);
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
      const replacements = await emitBuildResources(
        this,
        plan.originalCss,
        plan.resources,
        settings.assetsDirectory,
      );
      const css = resolveResources(plan.originalCss, replacements);
      /**
       * Emits a stylesheet using this build's environment, asset directory, and map settings.
       * @param text - Final CSS with resource placeholders replaced.
       * @param sourceMap - Optional serialized map for the final CSS.
       * @returns The bundler's reference for the emitted asset.
       */
      const emitCss = (text: string, sourceMap?: string) =>
        emitBuildCss(project, name, this, text, sourceMap);
      /**
       * Builds the stylesheet URL recorded in the document manifest.
       * @param reference - Asset reference already issued by this build context.
       * @returns The emitted filename prefixed by the configured public base.
       */
      const href = (reference: string) => settings.publicBase + this.getFileName(reference);
      const revision = createHash("sha256")
        .update(plan.designSystem)
        .update("\0")
        .update(plan.originalCss)
        .update("\0")
        .update(JSON.stringify(plan.producers))
        .update("\0")
        .digest("hex");
      const output: BuildOutput = {};
      if (config.emission !== "split") {
        const mapped = settings.sourceMaps
          ? decodeCssSourceMap(
              cssForBrowser(
                name,
                {
                  css: plan.originalCss,
                  sourceMap: plan.sourceMap,
                },
                replacements,
              ),
            )
          : undefined;
        output.buildCss = await emitCss(css, mapped?.map ? JSON.stringify(mapped.map) : undefined);
        output.manifest = {
          version: 1,
          revision,
          parts: [{ id: "document", href: href(output.buildCss) }],
          modules: Object.fromEntries(
            plan.producers.map((producer) => [producer.module, ["document"]]),
          ),
        };
      } else {
        const references = new Map<string, string>();
        const parts: {
          /** Occurrence identity preserving partition position even when content shares an asset. */
          id: string;
          /** Public URL of the emitted partition asset. */
          href: string;
        }[] = [];
        const partitions = partitionStylesheet(plan.rawParts);
        for (const [index, part] of partitions.entries()) {
          let reference = references.get(part.key);
          if (!reference) {
            const mapped = settings.sourceMaps
              ? decodeCssSourceMap(cssForBrowser(name, part, replacements))
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
          [...inputs(name).values()].map((input) => [input.moduleId, input.dependencies]),
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
        output.manifest = { version: 1, revision, parts, modules };
      }
      project.setBuildOutput(name, output);
      this.emitFile({
        type: "asset",
        fileName: "stillwind-manifest.json",
        source: JSON.stringify(output.manifest, null, 2),
      });
    },
    /**
     * Adds CSS metadata or a split stylesheet loader to a rendered production chunk.
     *
     * @param code - Rendered JavaScript for this chunk.
     * @param chunk - Chunk metadata, updated with the selected CSS files.
     * @returns A rewritten chunk with a null map for split CSS, otherwise `null`.
     */
    renderChunk(code, chunk) {
      return renderChunkStyles(project, this.environment?.name, this, code, chunk);
    },
  };
}
