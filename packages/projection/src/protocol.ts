/** Original document format used to select direct parsing or a component converter. */
export type SourceFormat = "script" | "jsx" | "astro" | "svelte" | "vue";

/** Editable source text and the source identities needed to map output back to it. */
export interface SourceDocument {
  /** Absolute document URI supplied by the host, including for virtual documents. */
  readonly uri: string;
  /** Original text whose UTF-16 offsets are used by projection ranges. */
  readonly code: string;
  /** Format of this text, independent of the selected output framework. */
  readonly format: SourceFormat;
  /** Serialized source map from this document to the source that produced it, when available. */
  readonly upstreamMap?: string;
  /** Exact source name used in emitted maps. Preparation defaults it to `uri` when omitted. */
  readonly sourceMapSource?: string;
}

/** Parser settings for the accompanying script buffer. */
export interface ScriptSyntax {
  /** Syntax accepted by the parser. `dts` selects TypeScript declaration syntax. */
  language: "js" | "jsx" | "ts" | "tsx" | "dts";
  /** Module parsing mode. `unambiguous` lets the parser infer the mode from the source. */
  moduleKind: "module" | "commonjs" | "script" | "unambiguous";
}

/** Host identities and compilation options carried through source preparation. */
export interface ProjectMetadata {
  /** Full host identifier used to resolve imports and read this source, including query suffixes. */
  resolverId: string;
  /** Absolute or virtual filesystem name used by TypeScript to resolve this source's imports. */
  checkerFileName: string;
  /** Filename passed to component converters and recorded in their source maps. */
  projectionFileName: string;
  /** Stable identity used by the compiler and project session. Must identify one resolver source. */
  moduleId: string;
  /** Source framework constraint for component projections. Macro outputs remain independent. */
  framework?: "react" | "preact" | "solid" | "astro" | "svelte" | "vue" | "vanilla";
  /** Stylesheet module specifiers injected into a verified macro import realm, in import order. */
  stylesheetImports?: string[];
  /** Module exporting CSS to replace an explicit `stillwind/module-css` import. */
  moduleCssImport?: string;
}

/** Document and host metadata supplied to `prepareSource` before parsing or conversion. */
export interface ProjectInput extends ProjectMetadata {
  /** Original document retained for diagnostics and compiler edits after conversion. */
  document: SourceDocument;
  /** Parser settings for direct documents. Component converters supply their own settings. */
  syntax: ScriptSyntax;
}

/** Parser input that retains the original document for diagnostics and edits. */
export type PreparedSource = ProjectMetadata &
  (
    | {
        /** Selects parsing of the original document without a component projection. */
        kind: "direct";
        /** Document whose text is parsed and edited directly. */
        document: SourceDocument & {
          /** Restricts direct input to script formats that require no component converter. */
          readonly format: Extract<SourceFormat, "script" | "jsx">;
        };
        /** Parser settings for `document.code`. */
        syntax: ScriptSyntax;
      }
    | {
        /** Selects parsing of generated code and mapping edits back to the component. */
        kind: "projected";
        /** Original component retained as the target of diagnostics and edits. */
        document: SourceDocument & {
          /** Component format that selects the converter used for this projection. */
          readonly format: Extract<SourceFormat, "vue" | "svelte" | "astro">;
        };
        /** Generated parser input and its correspondence to the original component. */
        projection: SourceProjection;
      }
  );

/** Converter identity used to distinguish projections produced by different versions or options. */
export interface ConverterIdentity {
  /** Nonblank package name of the converter. */
  name: string;
  /** Nonblank version read from the converter's package metadata. */
  version: string;
  /**
   * Nonblank fingerprint that must change when settings change the projection. `converterIdentity`
   * uses a SHA-256 hex digest of serialized options, including their property order.
   */
  optionsFingerprint: string;
}

/** Generated parser input and the original ranges in which the compiler may apply edits. */
export interface SourceProjection {
  /** Generated script text addressed by the generated side of `map`. */
  code: string;
  /** Parser settings for the generated `code`. */
  syntax: ScriptSyntax;
  /** Global declaration text loaded into a virtual `.d.ts` file by the checker, when needed. */
  checkerGlobals?: string;
  /** Correspondence from generated code positions back to the original document. */
  map: ProjectionMap;
  /** Package version and options used to produce this projection. */
  converter: ConverterIdentity;
  /** Original script ranges whose imports are visible within each realm and its child realms. */
  importRealms: {
    /** Identifier referenced by editable regions and child realms. Must be unique here. */
    id: number;
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, at or after `start`. */
    end: number;
    /** Parent realm identifier for inherited imports. References must form an acyclic graph. */
    parent?: number;
  }[];
  /** Nonoverlapping original ranges eligible for edits, with import and quote context. */
  editableRegions: {
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, at or after `start`. */
    end: number;
    /** Identifier of the realm whose imports are visible in this range. */
    importRealm: number;
    /** Surrounding HTML attribute quote that generated expressions must preserve, when present. */
    attributeQuote?: "single" | "double";
  }[];
  /** Astro client script bodies checked independently, in original source order, when present. */
  checkerScriptRanges?: {
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, strictly after `start`. */
    end: number;
  }[];
}

/** Correspondence from generated JS/TS back to the original document, validated by the compiler. */
export type ProjectionMap =
  | {
      /** Selects version 3 source-map decoding. */
      kind: "v3";
      /** Serialized version 3 map whose generated positions address `SourceProjection.code`. */
      json: string;
      /** Exact entry in the map's `sources` array that identifies the original document. */
      source: string;
    }
  | {
      /** Selects Volar interval decoding. */
      kind: "volar";
      /**
       * Batches of corresponding spans paired by array index. Present arrays in each batch must
       * have equal lengths. Records need not be globally sorted. Offsets and lengths must be
       * nonnegative integers fitting `u32`, and spans must stay within their respective buffers.
       */
      mappings: {
        /** Inclusive UTF-16 offsets in the original document. */
        sourceOffsets: number[];
        /** Inclusive UTF-16 offsets in `SourceProjection.code`. */
        generatedOffsets: number[];
        /** Original span lengths in UTF-16 units. Zero represents a boundary anchor. */
        lengths: number[];
        /** Generated span lengths in UTF-16 units. Omission reuses `lengths`. */
        generatedLengths?: number[];
      }[];
    };
