import { decodeCssSourceMap, type DecodedCssSourceMap } from "@stillwind/host";
import type { NativeStylesheetPlan } from "@stillwind/native";

import { cssForBrowser, type BrowserCssSources } from "./browser-css.ts";
import type { CssContext, DesignSystem } from "./design-system.ts";
import { readableFile } from "./file-access.ts";
import type { ProjectSettings } from "./project.ts";
import {
  resolvedFile,
  resolveResources,
  resourceReplacements,
  type StylesheetResource,
} from "./resources.ts";

/**
 * A rendered stylesheet with resolved resources and browser-ready parts, retaining native
 * provenance and coverage.
 */
export interface StylesheetPlan extends Omit<NativeStylesheetPlan, "css" | "resources" | "parts"> {
  /** Environment revision when this selection was planned. */
  revision: number;
  /** CSS with compiler resource placeholders retained for production asset emission. */
  originalCss: string;
  /** Resource placeholders mapped to resolved identities and readable absolute paths. */
  resources: Map<string, StylesheetResource>;
  /** Placeholder tokens mapped to escaped URLs used in resolved CSS. */
  resourceReplacements: Map<string, string>;
  /** Document CSS with resource placeholders replaced by readable resource URLs. */
  css: string;
  /** Native occurrences in cascade order, retaining placeholders and native source maps. */
  rawParts: NativeStylesheetPlan["parts"];
  /** Resolved occurrences in the same order as `rawParts`, ready for development delivery. */
  parts: ResolvedStylesheetPart[];
}

/** A stylesheet occurrence whose resources and optional map are ready for the browser. */
export interface ResolvedStylesheetPart {
  /** Native content fingerprint used by the runtime to share equal CSS payloads. */
  key: string;
  /** Compiler module identities that activate this occurrence. Empty means shared support CSS. */
  producers: string[];
  /** CSS with resolved resource URLs, excluding the inline source-map comment. */
  css: string;
  /**
   * Public source map when available. Undefined when development maps are disabled, or null when
   * the CSS contains no encoded map.
   */
  map: DecodedCssSourceMap["map"] | undefined;
}

/**
 * Plans selected compiler modules and resolves their stylesheet resource placeholders.
 *
 * Referenced resources are checked for readability and registered as watch dependencies.
 * Development parts include public source maps only when development CSS maps are enabled.
 *
 * @param designSystem - Initialized design system for the current environment.
 * @param settings - Project root, source-map settings, and optional server used to validate
 * resource access.
 * @param sources - Environment sources used to compose browser CSS maps.
 * @param revision - Environment revision recorded in the resulting plan.
 * @param modules - Compiler module identities selected for document placement.
 * @param context - Vite resolver and watch operations for referenced resources.
 * @param complete - Whether the caller has collected the complete application graph. False uses
 * observed coverage.
 * @returns A promise for resolved CSS, raw compiler parts, browser-ready parts, and resource
 * replacements.
 * @throws If the compiler is uninitialized, stylesheet planning fails, or a resource cannot be
 * resolved or read.
 */
export async function resolveStylesheet(
  designSystem: DesignSystem,
  settings: ProjectSettings,
  sources: BrowserCssSources,
  revision: number,
  modules: string[],
  context: CssContext,
  complete = false,
): Promise<StylesheetPlan> {
  const plan = designSystem.compiler.native.planStylesheet(
    {
      modules,
      coverage: complete
        ? {
            kind: "complete",
            target: "application",
            revision: String(revision),
          }
        : {
            kind: "observed",
          },
      placement: "document",
    },
    settings.watching ? settings.devSourceMaps : settings.sourceMaps,
  );

  const resources = new Map(designSystem.resources);
  for (const resource of plan.resources) {
    const resolved = await context.resolve(
      resource.specifier,
      designSystem.compiler.resolverId(resource.moduleId),
      {
        skipSelf: true,
      },
    );
    const id = resolvedFile(resolved, `Stillwind resource ${resource.specifier}`);
    const file = await readableFile(id, settings.devServerRef);
    context.addWatchFile(file);
    resources.set(resource.placeholder, {
      id,
      file,
    });
  }

  const replacements = resourceReplacements(resources);

  return {
    ...plan,
    revision,
    originalCss: plan.css,
    resources,
    resourceReplacements: replacements,
    css: resolveResources(plan.css, replacements),
    rawParts: plan.parts,
    parts: plan.parts.map((part) => {
      const mapped =
        settings.watching && settings.devSourceMaps
          ? decodeCssSourceMap(cssForBrowser(sources, part, replacements))
          : undefined;
      return {
        key: part.key,
        producers: part.producers,
        css: mapped?.code ?? resolveResources(part.css, replacements),
        map: mapped?.map,
      };
    }),
  };
}
