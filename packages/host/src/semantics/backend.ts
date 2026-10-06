import type { NativeAssetRecord, NativeCheckerBinding, NativeTypeFact } from "@stillwind/native";
import type { CheckerBuffer, PreparedSource } from "@stillwind/projection";

/** Original UTF-8 expression range and its validated projected UTF-16 range. */
export interface CheckerSpan {
  /** Correspondence role matched when mapping a producer, expression, or module specifier. */
  role: string;
  /** Inclusive UTF-8 byte offset in the original document. */
  start: number;
  /** Exclusive UTF-8 byte offset in the original document, at or after `start`. */
  end: number;
  /** Inclusive UTF-16 offset in the unedited projected parser buffer. */
  projectedStart: number;
  /** Exclusive UTF-16 offset in the projected buffer, at or after `projectedStart`. */
  projectedEnd: number;
}

/** Original-coordinate requests for one module and its optional parser projection. */
export type TypeFactSource = PreparedSource & {
  /**
   * Original expressions queried for lowering or optional macro recognition. Every probe must
   * match an expression in a checker AST.
   */
  probes: readonly {
    /** Inclusive UTF-8 byte offset of the original expression. */
    start: number;
    /** Exclusive UTF-8 byte offset of the original expression. */
    end: number;
    /**
     * True makes unresolved identities optional for macro recognition. Otherwise missing or
     * unresolved types and nonfinite Stillwind candidates are errors.
     */
    recognitionOnly?: boolean;
  }[];
  /** Validated original-byte to projected-UTF-16 correspondences from the frontend. */
  checkerSpans?: readonly CheckerSpan[];
  /** Validated generated references that must use their global helper declarations. */
  checkerBindings?: readonly NativeCheckerBinding[];
  /** Alternative checker buffers, including independent Astro client modules. */
  checkerBuffers?: readonly CheckerBuffer[];
  /** Absolute framework declaration files included in the checker project. */
  checkerTypes?: readonly string[];
  /** Import resolutions used to rewrite checker module paths. */
  resolutions?: readonly {
    /**
     * Full resolved source identity. The checker rewrites this to the matching checker filename
     * when one is supplied.
     */
    id: string;
    /** Inclusive UTF-8 byte offset of the original module string literal, including its quotes. */
    start: number;
    /** Exclusive UTF-8 byte offset of the original module string literal. */
    end: number;
  }[];
};

/** An original UTF-8 range whose dependency or checker information could not be verified. */
export interface UnverifiedRange {
  moduleId: string;
  start?: number;
  end?: number;
  reason: string;
}

/** Captured declarations and source buffers supplied to a semantic backend. */
export interface SemanticInputs {
  overlayFiles?: ReadonlyMap<string, string>;
  ambientDeclarations?: ReadonlyMap<string, string>;
}

/** Available facts remain usable when other expressions cannot be verified. */
export interface AvailableFacts {
  facts: Map<string, NativeTypeFact[]>;
  unverified: UnverifiedRange[];
}

/** Semantic queries implemented by a Node checker or an embedding application's checker worker. */
export interface TypeFactsBackend {
  resolveModules(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ):
    | ReadonlyMap<string, ReadonlyMap<string, string>>
    | Promise<ReadonlyMap<string, ReadonlyMap<string, string>>>;
  collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, NativeTypeFact[]> | Promise<Map<string, NativeTypeFact[]>>;
  collectAvailable(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): AvailableFacts | Promise<AvailableFacts>;
  invalidate(): void | Promise<void>;
  close(): void | Promise<void>;
}
