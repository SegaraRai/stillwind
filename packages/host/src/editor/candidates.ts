import type { NativeEditorQuery } from "@stillwind/native";

/** Shares candidate previews, prepared ordering, and completion catalogs within one query. */
export class CandidateQueries {
  readonly #query: NativeEditorQuery;
  readonly #previews = new Map<string, string | null>();
  readonly #completions = new Map<string, string[]>();

  constructor(query: NativeEditorQuery) {
    this.#query = query;
  }

  preview(candidate: string): string | null {
    if (!this.#previews.has(candidate)) {
      this.#previews.set(candidate, this.#query.hoverCandidate(candidate));
    }
    return this.#previews.get(candidate)!;
  }

  compare(left: string, right: string): number | null {
    return this.#query.compareCandidates(left, right);
  }

  /**
   * Prepares all spellings in `values` with one native request.
   * The returned comparison accepts these spellings and returns null for unresolved
   * candidates. Its numeric ranks belong only to this batch.
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

  complete(prefix: string): string[] {
    if (!this.#completions.has(prefix)) {
      this.#completions.set(prefix, this.#query.completeCandidate(prefix));
    }
    return this.#completions.get(prefix)!;
  }

  color(candidate: string): number[] | undefined {
    return this.#query.candidateColor(candidate) ?? undefined;
  }
}

/** Splits only at delimiters outside brackets, parentheses, quoted text, and escapes. */
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
