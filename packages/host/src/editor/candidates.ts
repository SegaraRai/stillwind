import type { NativeCandidateContext, NativeEditorQuery } from "@stillwind/native";

/** Shares candidate previews, batch ordering, and completion catalogs for one captured design system. */
export class CandidateQueries {
  /** Immutable design system that owns every cached result in this instance. */
  readonly #query: NativeEditorQuery;
  /** Successful and failed previews keyed by input role and decoded candidate text. */
  readonly #previews = new Map<string, string | null>();
  /** Shared completion arrays keyed by input role and decoded prefix text. */
  readonly #completions = new Map<string, string[]>();

  /**
   * Creates empty caches for one captured design system.
   *
   * @param query - Native query whose theme, utility definitions, and compiler settings supply
   * all results. A different design system requires a new instance.
   */
  constructor(query: NativeEditorQuery) {
    this.#query = query;
  }

  /**
   * Caches standalone CSS for one candidate and its input role, including failed resolution.
   * The result uses `&` as its subject and omits enclosing source conditions and definitions.
   *
   * @param candidate - Decoded candidate text, including any variants supplied by the caller.
   * Option roles also accept the option text supported by their registry.
   * @param context - Input role used for validation and emission. Defaults to ordinary utilities.
   * @returns CSS for valid input, or null when the native query cannot emit it.
   * @throws If a matching ordinary utility definition has a malformed requirement.
   */
  preview(candidate: string, context: NativeCandidateContext = "utility"): string | null {
    const key = `${context}\0${candidate}`;
    if (!this.#previews.has(key)) {
      this.#previews.set(key, this.#query.previewCandidate(candidate, context));
    }
    return this.#previews.get(key)!;
  }

  /**
   * Compares two ordinary utility candidates using the compiler's variant and utility order.
   *
   * @param left - Full decoded candidate spelling, including its variants.
   * @param right - Full decoded candidate spelling, including its variants.
   * @returns -1, 0, or 1, or null when either candidate cannot resolve.
   * @throws If a matching utility definition has a malformed requirement.
   */
  compare(left: string, right: string): number | null {
    return this.#query.compareCandidates(left, right);
  }

  /**
   * Prepares all spellings in `values` with one native request.
   * Repeated spellings are resolved once. The input array remains unchanged.
   *
   * @param values - Full decoded ordinary utility candidates, including their variants.
   * @returns A comparator whose numeric rank differences belong only to this batch.
   * It returns null for unresolved candidates or spellings absent from `values`.
   * An empty batch returns a comparator that always returns null.
   * @throws If a matching utility requirement is malformed or a native rank exceeds uint32.
   */
  order(values: readonly string[]): (left: string, right: string) => number | null {
    const unique = [...new Set(values)];
    if (unique.length === 0) {
      return () => null;
    }
    const ranks = this.#query.candidateRanks(unique);
    const positions = new Map(unique.map((value, index) => [value, ranks[index]]));
    return (left, right) => {
      const a = positions.get(left);
      const b = positions.get(right);
      return a == null || b == null ? null : a - b;
    };
  }

  /**
   * Caches unique labels in lexical order for one decoded partial candidate and its role.
   * Existing variant prefixes remain present, and variant labels end in `:`.
   *
   * @param prefix - Decoded partial candidate, including any enclosing variants.
   * @param context - Input role that selects the candidate registry and permitted variants.
   * Defaults to ordinary utilities.
   * @returns A shared array of matching labels. Callers must keep the array unchanged.
   * @throws If a matching ordinary utility definition has a malformed requirement.
   */
  complete(prefix: string, context: NativeCandidateContext = "utility"): string[] {
    const key = `${context}\0${prefix}`;
    if (!this.#completions.has(key)) {
      this.#completions.set(key, this.#query.completeCandidate(prefix, context));
    }
    return this.#completions.get(key)!;
  }

  /**
   * Resolves a candidate's concrete color against this query's theme.
   *
   * @param candidate - Full decoded ordinary utility candidate, including variants and modifiers.
   * @returns sRGB red, green, blue, and effective alpha components in 0..1, or undefined when
   * the native query cannot determine one unambiguous concrete color.
   * @throws If a matching utility definition has a malformed requirement.
   */
  color(candidate: string): number[] | undefined {
    return this.#query.candidateColor(candidate) ?? undefined;
  }
}

/**
 * Splits at delimiters outside brackets, parentheses, single or double quotes, and escapes.
 * The operation preserves source spelling and does not validate nesting syntax.
 *
 * @param text - Candidate text whose backslashes escape the following UTF-16 code unit.
 * @param delimiter - One UTF-16 code unit, such as `:` or `/`, that is not a bracket,
 * parenthesis, quote, or backslash.
 * @returns Segments in their original order, including empty segments. Nothing is trimmed
 * or unescaped, and an empty input returns one empty segment.
 */
export function segments(text: string, delimiter: string): string[] {
  const result: string[] = [];
  let start = 0;
  let depth = 0;
  let quote = "";
  let escaped = false;
  for (let index = 0; index < text.length; index++) {
    const character = text[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) {
        quote = "";
      }
      continue;
    }
    if (character === "'" || character === '"') {
      quote = character;
    } else if (character === "[" || character === "(") {
      depth++;
    } else if (character === "]" || character === ")") {
      depth--;
    } else if (character === delimiter && depth === 0) {
      result.push(text.slice(start, index));
      start = index + 1;
    }
  }
  result.push(text.slice(start));
  return result;
}
