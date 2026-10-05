import { createHash } from "node:crypto";
import { isAbsolute } from "node:path";
import { pathToFileURL } from "node:url";

import type { EncodedSourceMap } from "@jridgewell/gen-mapping";
import {
  cleanModuleId,
  cssWithSourceMap,
  decodeCssSourceMap,
  type CssSourceMapInput,
} from "@stillwind/host";

import { browserSourceMap, type BrowserSourceMapInput } from "./browser-source-map.ts";
import type { ProducerInput } from "./compile.ts";
import { sourceModuleId } from "./module-ids.ts";

/** Project sources used to compose CSS maps and choose source names safe to send to a browser. */
export interface BrowserCssSources {
  /** Absolute project root used to compute public source names. */
  root: string;
  /**
   * Vite environment name. Only `client` allows ordinary script producers to retain
   * project-relative names.
   */
  environment: string;
  /** Absolute paths read by the design system and used as visible CSS sources. */
  cssFiles: ReadonlySet<string>;
  /**
   * Compiled producers and their upstream maps. SFCs and projections retain opaque public source
   * names.
   */
  inputs: readonly Readonly<ProducerInput>[];
}

/**
 * Rewrites CSS map sources without exposing absolute paths or embedded source text.
 *
 * Known CSS files inside the project and ordinary client scripts receive project-relative names.
 * All other source identities receive SHA-256 names.
 *
 * @param sources - Known CSS files and producer inputs for one project environment.
 * @param map - Encoded map object or serialized source map.
 * @returns An encoded map with positions preserved, no source text or symbol names, and approved or
 * hashed source names.
 */
export function publicCssSourceMap(
  sources: BrowserCssSources,
  map: BrowserSourceMapInput,
): EncodedSourceMap {
  const visible = new Map<string, string>();
  const add = (id: string) => {
    const relative = sourceModuleId(sources.root, cleanModuleId(id));
    if (relative !== ".." && !relative.startsWith("../") && !isAbsolute(relative)) {
      visible.set(pathToFileURL(cleanModuleId(id)).href, `stillwind-source:/${relative}`);
    }
  };
  for (const file of sources.cssFiles) {
    add(file);
  }
  if (sources.environment === "client") {
    for (const input of sources.inputs) {
      if (!input.projection && !/\.(?:astro|svelte|vue)$/.test(cleanModuleId(input.id))) {
        add(input.id);
      }
    }
  }
  const publicNames = new Set(visible.values());
  return browserSourceMap(map, (source) => {
    if (/^stillwind-source:[a-f0-9]{64}$/.test(source) || publicNames.has(source)) {
      return source;
    }
    return (
      visible.get(source) ?? `stillwind-source:${createHash("sha256").update(source).digest("hex")}`
    );
  });
}

/**
 * Replaces CSS resource placeholders and composes a source map for browser delivery.
 *
 * @param sources - Environment sources and upstream producer maps.
 * @param output - Compiler CSS and its native source map.
 * @param replacements - Map from resource placeholder tokens to escaped URL text.
 * @returns CSS with replaced resources and, when a map is available, an inline map containing
 * public source names.
 */
export function cssForBrowser(
  sources: BrowserCssSources,
  output: CssSourceMapInput,
  replacements: ReadonlyMap<string, string>,
): string {
  const decoded = decodeCssSourceMap(
    cssWithSourceMap(
      output,
      replacements,
      sources.inputs.flatMap((input) => (input.upstream ? [input.upstream] : [])),
      false,
    ),
  );
  if (!decoded.map) {
    return decoded.code;
  }

  const encoded = Buffer.from(JSON.stringify(publicCssSourceMap(sources, decoded.map))).toString(
    "base64",
  );
  return `${decoded.code}\n/*# sourceMappingURL=data:application/json;base64,${encoded} */`;
}
