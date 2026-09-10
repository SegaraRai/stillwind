export interface ProjectSource {
  id: string;
  path: string;
  source: string;
  language?: "js" | "jsx" | "ts" | "tsx";
  framework?: "React" | "Preact" | "Solid" | "Vue" | "Svelte" | "Astro";
  /** Source-to-upstream map for a normal input; it does not change the edit target. */
  map?: string;
  /** Host-allocated stylesheet modules placed inside a verified macro import realm. */
  stylesheet_imports?: string[];
  /** Original framework source; source above remains the projected JS/TS. */
  fw_code?: string;
  fw2js_map?: ProjectionMap;
  fw_map?: string;
  converter?: { name: string; version: string; options_fingerprint: string };
  /** Offsets in these host records are UTF-16, not Rust byte offsets. */
  import_realms?: Array<{ id: number; start: number; end: number; parent?: number }>;
  regions?: Array<{
    start: number;
    end: number;
    import_realm: number;
    attribute_quote?: "single" | "double";
  }>;
}

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
