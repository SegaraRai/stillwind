/**
 * A host input and its native compilation metadata. For plain input, source is
 * both the parser input and edit target. For projected input, source is JS/TS and
 * fw_code is the original framework buffer receiving edits.
 *
 * Projected inputs require language, framework, converter, and fw2js_map; upstream
 * mapping belongs in fw_map rather than map. Projection metadata is invalid without
 * fw_code (empty realm/region arrays are allowed).
 */
export interface ProjectSource {
  /** Host identifier used for resolution and reading. */
  id: string;
  /** Native module identity, conventionally root-relative with forward slashes. */
  path: string;
  source: string;
  language?: "js" | "jsx" | "ts" | "tsx";
  framework?: "React" | "Preact" | "Solid" | "Vue" | "Svelte" | "Astro";
  /** Source-to-upstream map for a normal input; it does not change the edit target. */
  map?: string;
  /** Host-allocated stylesheet modules placed inside a verified macro import realm. */
  stylesheet_imports?: string[];
  /** JavaScript module exporting CSS for an explicit stillwind/module-css import. */
  module_css_import?: string;
  /** Original framework source; source above remains the projected JS/TS. */
  fw_code?: string;
  /** Independent checker script regions, in original SFC UTF-16 code units. */
  checker_scripts?: Array<{ start: number; end: number }>;
  /** Maps projected source positions back to fw_code. */
  fw2js_map?: ProjectionMap;
  /** Source map from fw_code to its upstream source. */
  fw_map?: string;
  converter?: { name: string; version: string; options_fingerprint: string };
  /** Half-open UTF-16 ranges in fw_code; parent refers to a realm id, not an index. */
  import_realms?: Array<{ id: number; start: number; end: number; parent?: number }>;
  /** Editable half-open UTF-16 ranges in fw_code, with import scope and quote context. */
  regions?: Array<{
    start: number;
    end: number;
    import_realm: number;
    attribute_quote?: "single" | "double";
  }>;
}

/**
 * Mapping from projected JS/TS to the original framework buffer. In a v3 map,
 * source identifies the original source entry in json. Volar sourceOffsets address
 * fw_code; generatedOffsets address source, all in UTF-16 code units.
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
