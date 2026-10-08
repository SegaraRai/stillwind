import type {
  NativeAssetRecord,
  NativeCheckerBinding,
  NativeSourceIssue,
  NativeTypeFact,
} from "@stillwind/native";
import type { CheckerBuffer, PreparedSource } from "@stillwind/projection";

/**
 * Failure to map, validate, or recover declaration identities for one source. The compiler turns
 * this error into a source report using its original-coordinate range.
 */
export class TypeFactsError extends Error {
  /** Compiler module identity used to locate the original prepared source. */
  readonly moduleId: string;
  /**
   * Inclusive original UTF-8 byte offset when the checker can identify it. Omission permits a
   * document-scoped report.
   */
  readonly start?: number;
  /**
   * Exclusive original UTF-8 byte offset when known. The compiler uses a source range only when
   * both endpoints are present.
   */
  readonly end?: number;
  /** Issue category and detail passed to native source-failure reporting. */
  readonly issue: NativeSourceIssue;

  /**
   * Records a checker contract failure in original source coordinates.
   *
   * @param moduleId - Compiler identity of the affected source.
   * @param start - Optional inclusive original UTF-8 byte offset.
   * @param end - Optional exclusive original UTF-8 byte offset.
   * @param kind - Native issue category, such as checker mapping, assignment, or identity
   * resolution.
   * @param detail - Error message also stored as the issue detail.
   */
  constructor(
    moduleId: string,
    start: number | undefined,
    end: number | undefined,
    kind: NativeSourceIssue["kind"],
    detail: string,
  ) {
    super(detail);
    this.moduleId = moduleId;
    this.start = start;
    this.end = end;
    this.issue = {
      kind,
      detail,
    };
  }
}

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
  /** Compiler module identity of the original document. */
  moduleId: string;
  /** Inclusive original UTF-8 byte offset. Omission identifies unavailable information for the module. */
  start?: number;
  /** Exclusive original UTF-8 byte offset when a specific expression range is known. */
  end?: number;
  /** Explanation of the missing dependency, checker result, or source correspondence. */
  reason: string;
}

/** Captured declarations and source buffers supplied to a semantic backend. */
export interface SemanticInputs {
  /** Captured original text keyed by absolute checker file path. Entries override filesystem reads. */
  overlayFiles?: ReadonlyMap<string, string>;
  /** Ambient declaration text keyed by absolute checker file path, supplied without writing to disk. */
  ambientDeclarations?: ReadonlyMap<string, string>;
}

/** Available facts remain usable when other expressions cannot be verified. */
export interface AvailableFacts {
  /** Available native facts grouped by compiler module identity. */
  facts: Map<string, NativeTypeFact[]>;
  /** Ranges for queries that could not be verified. Facts for other queries remain usable. */
  unverified: UnverifiedRange[];
}

/** Semantic queries implemented by a Node checker or an embedding application's checker worker. */
export interface TypeFactsBackend {
  /**
   * Resolves imports using the checker environment before source-provider fallback resolution.
   *
   * @param sources - Prepared sources with original probes and checker correspondences.
   * @param assets - Compiler declaration metadata grouped by module identity.
   * @param inputs - Captured overlays and ambient declarations for the same analysis.
   * @returns Import request to resolved identity maps, grouped by source resolver identity.
   * @throws If checker setup or resolution cannot complete.
   */
  resolveModules(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ):
    | ReadonlyMap<string, ReadonlyMap<string, string>>
    | Promise<ReadonlyMap<string, ReadonlyMap<string, string>>>;
  /**
   * Collects semantic facts and validates declaration contracts for required probes.
   *
   * @param sources - Prepared sources with original UTF-8 probe ranges.
   * @param assets - Compiler declarations grouped by module identity.
   * @param inputs - Captured overlays and declarations. Omission permits backend reads.
   * @returns Native facts grouped by compiler module identity.
   * @throws If a required type, declaration identity, or source correspondence cannot be verified.
   */
  collect(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): Map<string, NativeTypeFact[]> | Promise<Map<string, NativeTypeFact[]>>;
  /**
   * Keeps verified facts while recording unavailable queries.
   *
   * @param sources - Prepared sources with original UTF-8 probe ranges.
   * @param assets - Compiler declarations grouped by module identity.
   * @param inputs - Captured overlays and declarations for this analysis.
   * @returns Facts for successful queries and original ranges with explanations for other queries.
   * @throws If backend setup or transport fails before partial results can be produced.
   */
  collectAvailable(
    sources: readonly TypeFactSource[],
    assets: ReadonlyMap<string, readonly NativeAssetRecord[]>,
    inputs?: SemanticInputs,
  ): AvailableFacts | Promise<AvailableFacts>;
  /**
   * Discards cached filesystem state before subsequent queries. Call between analysis transactions.
   *
   * @returns Nothing, or a promise that resolves after invalidation.
   */
  invalidate(): void | Promise<void>;
  /**
   * Releases checker resources after pending queries complete.
   *
   * @returns Nothing, or a promise that resolves after cleanup.
   * @throws If checker cleanup fails.
   */
  close(): void | Promise<void>;
}
