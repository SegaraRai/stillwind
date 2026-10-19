import {
  completionDetail,
  type NativeAddedMarker,
  type NativeCandidateCompletion,
  type NativeCandidateContext,
  type NativeEditorQuery,
} from "@stillwind/native";

/**
 * Shares previews, batch ordering, completion catalogs, and summaries for one captured design system.
 */
export class CandidateQueries {
  /** Native query pinned to the design system used for candidate validation and previews. */
  readonly #query: NativeEditorQuery;
  /** Successful and failed CSS previews keyed by preview kind, input role, and decoded candidate. */
  readonly #previews = new Map<string, string | null>();
  /** Shared completion arrays keyed by input role, decoded prefix, and snippet support. */
  readonly #completions = new Map<string, readonly NativeCandidateCompletion[]>();
  /** Added marker classes and null results keyed by input role and decoded candidate. */
  readonly #markers = new Map<string, NativeAddedMarker | null>();
  /** Summaries and empty results keyed by the full generated CSS, including preview annotations. */
  readonly #details = new Map<string, string | undefined>();

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
   * Ordinary utilities use `&` as their subject. Previews omit enclosing source conditions.
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
   * Caches the marker class that one candidate adds to the element as written.
   *
   * @param candidate - Decoded candidate text, including enclosing variants.
   * @param context - Input role of the candidate. Only ordinary utilities add marker classes.
   * @returns The class and its origins, or null when the candidate adds no marker class.
   * Ref conditions of an enclosing style object are not checked.
   */
  marker(candidate: string, context: NativeCandidateContext = "utility"): NativeAddedMarker | null {
    const key = `${context}\0${candidate}`;
    if (!this.#markers.has(key)) {
      this.#markers.set(key, this.#query.addedMarker(candidate, context));
    }
    return this.#markers.get(key)!;
  }

  /**
   * Caches the selectors, at-rule conditions, and declarations of a variant chain.
   *
   * @param candidate - Decoded variant chain, with or without a trailing colon.
   * @param context - Original input role whose constraints must allow the entire chain.
   * @returns CSS introduced by the variants, or null when the chain or role is unsupported.
   * Utility output and surrounding source conditions are omitted.
   */
  variant(candidate: string, context: NativeCandidateContext): string | null {
    const key = `variant\0${context}\0${candidate}`;
    if (!this.#previews.has(key)) {
      this.#previews.set(key, this.#query.previewVariant(candidate, context));
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
   * Suggestions do not append a variant already present in the prefix. An ordinary utility
   * prefix without variants, `!`, or `-` also receives marker class names, which may have no
   * preview.
   *
   * @param prefix - Decoded partial candidate, including any enclosing variants.
   * @param context - Input role that selects the candidate registry and permitted variants.
   * Defaults to ordinary utilities.
   * @param snippets - Whether to include empty brackets for arbitrary variant values and `name/`
   * labels for modifiable markers. Defaults to false.
   * @returns A shared readonly array of matching labels.
   * @throws If a matching ordinary utility definition has a malformed requirement.
   */
  complete(
    prefix: string,
    context: NativeCandidateContext = "utility",
    snippets = false,
  ): readonly NativeCandidateCompletion[] {
    const key = `${context}\0${prefix}\0${snippets}`;
    if (!this.#completions.has(key)) {
      this.#completions.set(key, this.#query.completeCandidate(prefix, context, snippets));
    }
    return this.#completions.get(key)!;
  }

  /**
   * Caches a short declaration or rule summary for selected completion CSS.
   *
   * @param css - Generated CSS to summarize, including optional preview annotations.
   * @returns A summary of at most 160 Unicode code points, or undefined when the CSS has
   * no declarations or descriptive rule wrappers.
   */
  detail(css: string): string | undefined {
    if (!this.#details.has(css)) {
      this.#details.set(css, completionDetail(css) ?? undefined);
    }
    return this.#details.get(css);
  }

  /**
   * Resolves a color utility's concrete color against this query's theme.
   *
   * @param candidate - Full decoded ordinary utility candidate, including variants and modifiers.
   * @returns sRGB red, green, blue, and effective alpha components in 0..1, or undefined when
   * the candidate is not a color utility or has no unambiguous concrete color.
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

/**
 * Describes an added marker class and the definitions that make it a marker.
 *
 * @param marker - Marker class returned by {@link CandidateQueries.marker}.
 * @returns Markdown with the class name first, followed by one line for each kind of origin.
 */
export function markerDocumentation(marker: NativeAddedMarker): string {
  const code = (text: string) => {
    const ticks = "`".repeat(
      Math.max(0, ...[...text.matchAll(/`+/g)].map((match) => match[0].length)) + 1,
    );
    return `${ticks}${text.startsWith("`") || text.endsWith("`") ? ` ${text} ` : text}${ticks}`;
  };
  const definitions = marker.origins.flatMap((origin) =>
    origin.kind === "utility" || origin.kind === "variant"
      ? [`the ${code(origin.name!)} ${origin.kind}`]
      : [],
  );
  const kinds = new Set(marker.origins.map((origin) => origin.kind));
  return [
    `Adds the marker class ${code(marker.class)} to the element as written.`,
    kinds.has("declared") ? "A `@stillwind-marker` statement declares this class." : undefined,
    definitions.length ? `Selectors in ${list(definitions)} select this class.` : undefined,
    kinds.has("source") ? "Selectors in plugin CSS select this class." : undefined,
  ]
    .filter(Boolean)
    .join("\n\n");
}

/**
 * Joins phrases into an English list.
 *
 * @param items - At least one phrase, in display order.
 * @returns The phrases separated by commas, with "and" before the last one.
 */
function list(items: readonly string[]): string {
  return items.length < 3
    ? items.join(" and ")
    : `${items.slice(0, -1).join(", ")}, and ${items.at(-1)}`;
}
