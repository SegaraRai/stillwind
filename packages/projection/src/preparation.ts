import type { ConverterIdentity } from "./protocol.ts";

/** Script body in the original component that defines a scope for imports. */
export interface ScriptRegion {
  /** Realm identifier used by editable regions. Must be unique within the component. */
  id: number;
  /** Inclusive UTF-16 offset in the original document, at a complete character boundary. */
  start: number;
  /** Exclusive UTF-16 offset, at or after `start` and at a complete character boundary. */
  end: number;
  /** Parent realm identifier whose imports this script can use, when present. */
  parent?: number;
}

/** A recognized component syntax or projection failure that the host can report as a diagnostic. */
export class PreparationError extends Error {
  /** Failure category and the message exposed to the host. */
  readonly issue: {
    /** `syntax` identifies rejected component syntax. `projection` identifies unusable conversion. */
    kind: "syntax" | "projection";
    /** Diagnostic text also used as this error's message. */
    detail: string;
  };
  /** Converter that produced the failure, when its identity is available. */
  readonly converter?: ConverterIdentity;
  /** Validated original range for a diagnostic. Omission makes the diagnostic document-wide. */
  readonly range?: {
    /** Inclusive UTF-16 offset in the original document. */
    start: number;
    /** Exclusive UTF-16 offset in the original document, at or after `start`. */
    end: number;
  };

  /**
   * Records a preparation failure without changing the supplied metadata.
   *
   * @param issue - Failure category and diagnostic text. `detail` becomes the error message.
   * @param converter - Converter identity, when known.
   * @param range - Optional original UTF-16 range already validated by `originalRange` or the caller.
   */
  constructor(
    issue: PreparationError["issue"],
    converter?: ConverterIdentity,
    range?: PreparationError["range"],
  ) {
    super(issue.detail);

    this.issue = issue;
    this.converter = converter;
    this.range = range;
  }
}

/**
 * Validates diagnostic bounds against the original text, including surrogate pair boundaries.
 *
 * @param source - Original document text.
 * @param start - Inclusive UTF-16 offset, or an omitted parser position.
 * @param end - Exclusive UTF-16 offset, or an omitted parser position.
 * @returns The bounds, including an empty range, or undefined for missing, noninteger, reversed,
 * out-of-bounds positions or positions inside a surrogate pair.
 */
export function originalRange(source: string, start?: number, end?: number) {
  if (
    start == null ||
    end == null ||
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    end < start ||
    end > source.length
  ) {
    return undefined;
  }

  /**
   * Checks whether an offset can delimit a range without splitting a surrogate pair.
   * @param offset - Validated integer UTF-16 offset between zero and the source length, inclusive.
   * @returns Whether the offset lies outside a high-surrogate/low-surrogate pair.
   */
  const boundary = (offset: number) =>
    offset === 0 ||
    offset === source.length ||
    !(
      source.charCodeAt(offset - 1) >= 0xd800 &&
      source.charCodeAt(offset - 1) <= 0xdbff &&
      source.charCodeAt(offset) >= 0xdc00 &&
      source.charCodeAt(offset) <= 0xdfff
    );
  if (!boundary(start) || !boundary(end)) {
    return undefined;
  }

  return {
    start,
    end,
  };
}
