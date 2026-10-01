/**
 * Format of the original document, independent of its output adapter.
 */
export type SourceFormat = "script" | "jsx" | "astro" | "svelte" | "vue";

/**
 * Original editable document and its mapping to upstream source.
 */
export interface SourceDocument {
  /**
   * Absolute URI supplied by the host.
   */
  readonly uri: string;
  readonly code: string;
  readonly format: SourceFormat;
  /**
   * Maps original document positions to upstream source positions.
   */
  readonly upstreamMap?: string;
  /**
   * Exact source name for stylesheet maps; preparation defaults it to uri.
   */
  readonly sourceMapSource?: string;
}

/**
 * Parser configuration for the code buffer supplied alongside it.
 */
export interface ScriptSyntax {
  language: "js" | "jsx" | "ts" | "tsx" | "dts";
  moduleKind: "module" | "commonjs" | "script" | "unambiguous";
}

/**
 * Host identities and compilation options shared by inputs and prepared sources.
 */
export interface ProjectMetadata {
  /**
   * Host identifier used for resolution and reading.
   */
  resolverId: string;
  /**
   * Absolute or virtual filesystem name used only by the TypeScript checker.
   */
  checkerFileName: string;
  /**
   * Filename supplied to framework projection libraries and their source maps.
   */
  projectionFileName: string;
  /**
   * Stable module identity used by the compiler and project session.
   */
  moduleId: string;
  /**
   * Explicit output adapter constraint; filename inference leaves it unset.
   */
  framework?: "react" | "preact" | "solid" | "astro" | "svelte" | "vue";
  /**
   * Host-allocated stylesheet modules placed inside a verified macro import realm.
   */
  stylesheetImports?: string[];
  /**
   * JavaScript module exporting CSS for an explicit stillwind/module-css import.
   */
  moduleCssImport?: string;
}

/**
 * Document awaiting preparation; syntax configures direct parsing.
 */
export interface ProjectInput extends ProjectMetadata {
  document: SourceDocument;
  syntax: ScriptSyntax;
}

/**
 * Prepared parser input with its original editable document.
 */
export type PreparedSource = ProjectMetadata &
  (
    | {
        kind: "direct";
        document: SourceDocument & {
          readonly format: Extract<SourceFormat, "script" | "jsx">;
        };
        syntax: ScriptSyntax;
      }
    | {
        kind: "projected";
        document: SourceDocument & {
          readonly format: Extract<SourceFormat, "vue" | "svelte" | "astro">;
        };
        projection: SourceProjection;
      }
  );

/**
 * Converter version and fingerprint of its serialized options.
 */
export interface ConverterIdentity {
  name: string;
  version: string;
  optionsFingerprint: string;
}

/**
 * Generated parser input and coordinates for editing its original document.
 */
export interface SourceProjection {
  code: string;
  syntax: ScriptSyntax;
  /**
   * Maps generated code positions back to the original document.
   */
  map: ProjectionMap;
  converter: ConverterIdentity;
  /**
   * Half-open original UTF-16 ranges; parent refers to a realm id.
   */
  importRealms: {
    id: number;
    start: number;
    end: number;
    parent?: number;
  }[];
  /**
   * Editable half-open original UTF-16 ranges with import scope and quote context.
   */
  editableRegions: {
    start: number;
    end: number;
    importRealm: number;
    attributeQuote?: "single" | "double";
  }[];
  /**
   * Independent checker script regions in original UTF-16 code units.
   */
  checkerScriptRanges?: Array<{ start: number; end: number }>;
}

/**
 * Maps generated JS/TS to the original document. In a v3 map, source identifies
 * the original source entry in json. Volar sourceOffsets address document.code;
 * generatedOffsets address projection.code, all in UTF-16 code units.
 *
 * Each Volar record uses parallel arrays of equal length. generatedLengths may be
 * omitted to reuse lengths. Array indices pair spans; outer records need not be
 * globally sorted. Offsets and lengths must be nonnegative integers fitting u32.
 */
export type ProjectionMap =
  | {
      kind: "v3";
      json: string;
      source: string;
    }
  | {
      kind: "volar";
      mappings: {
        sourceOffsets: number[];
        generatedOffsets: number[];
        lengths: number[];
        generatedLengths?: number[];
      }[];
    };
