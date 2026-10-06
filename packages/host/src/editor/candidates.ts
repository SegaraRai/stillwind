import type { NativeEditorQuery } from "@stillwind/native";

/** Shares candidate previews and completion catalogs within an immutable design system. */
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

  canReorderStyles(left: string, right: string): boolean {
    return this.#query.canReorderStyles(left, right);
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
  let start = 0,
    depth = 0,
    quote = "",
    escaped = false;
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
