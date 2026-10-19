import { segments } from "./candidates.ts";

/**
 * Converts one empty arbitrary-value placeholder into an LSP snippet.
 * @param candidate - Decoded replacement text, including enclosing variants.
 * @param source - Encodes decoded text for its source string. CSS uses the unchanged text.
 * @returns A snippet with its first tab stop inside `[]`, or undefined without a placeholder.
 */
export function completionSnippet(
  candidate: string,
  source: (text: string) => string = (text) => text,
): string | undefined {
  const text = candidate.endsWith(":") ? candidate.slice(0, -1) : candidate;
  const variant = segments(text, ":").at(-1)!;
  const placeholder = variant.lastIndexOf("[]");
  if (placeholder < 0) {
    return undefined;
  }
  const index = text.length - variant.length + placeholder;
  return `${escape(source(candidate.slice(0, index + 1)))}\${1}${escape(source(candidate.slice(index + 1)))}\${0}`;
}

/**
 * Converts the start of a modifiable marker's modifier into an LSP snippet.
 * @param label - Decoded marker name followed by `/`.
 * @param source - Encodes decoded text for its source string.
 * @returns A snippet that inserts `label` and selects a `name` placeholder after it.
 */
export function markerModifierSnippet(
  label: string,
  source: (text: string) => string = (text) => text,
): string {
  return `${escape(source(label))}\${1:name}`;
}

function escape(part: string): string {
  return part.replace(/[\\$}]/g, "\\$&");
}
