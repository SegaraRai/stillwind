/**
 * A host input and its native compilation metadata. For plain input, source is
 * both the parser input and edit target. For projected input, source is JS/TS and
 * fwCode is the original framework buffer receiving edits.
 *
 * Projected inputs require language, sourceFormat (vue, svelte, or astro), converter,
 * and fw2JsMap. Upstream mapping belongs in fwMap rather than map. Projection
 * metadata is invalid without fwCode (empty realm/region arrays are allowed).
 */
export interface ProjectSource {
  /** Host identifier used for resolution and reading. */
  resolverId: string;
  /** Absolute or virtual filesystem name used only by the TypeScript checker. */
  checkerFileName: string;
  /** Filename supplied to framework projection libraries and their source maps. */
  projectionFileName: string;
  /** Absolute URI of the original editable document, supplied by its host. */
  uri: string;
  /** Stable module identity used by the compiler and project session. */
  moduleId: string;
  source: string;
  language: "js" | "jsx" | "ts" | "tsx" | "dts";
  moduleKind: "module" | "commonjs" | "script" | "unambiguous";
  /** Original input format, retained when source becomes projected JS/TS. */
  sourceFormat?: "script" | "jsx" | "vue" | "svelte" | "astro";
  /** Exact source name written to generated stylesheet maps. */
  sourceMapSource?: string;
  /** Explicit output adapter constraint; filename inference leaves it unset. */
  framework?: "react" | "preact" | "solid" | "vue" | "svelte" | "astro";
  /** Source-to-upstream map for a normal input; it does not change the edit target. */
  map?: string;
  /** Host-allocated stylesheet modules placed inside a verified macro import realm. */
  stylesheetImports?: string[];
  /** JavaScript module exporting CSS for an explicit stillwind/module-css import. */
  moduleCssImport?: string;
  /** Original framework source; source above remains the projected JS/TS. */
  fwCode?: string;
  /** Independent checker script regions, in original SFC UTF-16 code units. */
  checkerScripts?: Array<{ start: number; end: number }>;
  /** Maps projected source positions back to fwCode. */
  fw2JsMap?: ProjectionMap;
  /** Source map from fwCode to its upstream source. */
  fwMap?: string;
  converter?: { name: string; version: string; optionsFingerprint: string };
  /** Half-open UTF-16 ranges in fwCode; parent refers to a realm id, not an index. */
  importRealms?: Array<{ id: number; start: number; end: number; parent?: number }>;
  /** Editable half-open UTF-16 ranges in fwCode, with import scope and quote context. */
  regions?: Array<{
    start: number;
    end: number;
    importRealm: number;
    attributeQuote?: "single" | "double";
  }>;
}

/**
 * Mapping from projected JS/TS to the original framework buffer. In a v3 map,
 * source identifies the original source entry in json. Volar sourceOffsets address
 * fwCode; generatedOffsets address source, all in UTF-16 code units.
 *
 * Each Volar record uses parallel arrays of equal length. generatedLengths may be
 * omitted to reuse lengths. Array indices pair spans; outer records need not be
 * globally sorted. Offsets and lengths must be nonnegative integers fitting u32.
 */
export type ProjectionMap =
  | { kind: "v3"; json: string; source: string }
  | {
      kind: "volar";
      mappings: Array<{
        sourceOffsets: number[];
        generatedOffsets: number[];
        lengths: number[];
        generatedLengths?: number[];
      }>;
    };
