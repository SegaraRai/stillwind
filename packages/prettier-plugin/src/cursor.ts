import type { RewriteResult } from "@stillwind/analysis/node";

/**
 * Maps an original UTF-16 cursor offset through nonoverlapping edits ordered by start.
 * Negative offsets return unchanged. A cursor follows a token whose spelling occurs once in
 * its replacement. Otherwise, the cursor keeps its relative position within the replacement.
 */
export function mapOffset(offset: number, source: string, edits: RewriteResult["edits"]): number {
  if (offset < 0) {
    return offset;
  }
  let delta = 0;
  for (const edit of edits) {
    if (offset < edit.start) {
      break;
    }
    if (offset >= edit.end) {
      delta += edit.newText.length - (edit.end - edit.start);
      continue;
    }
    const previous = source.slice(edit.start, edit.end);
    const token = [...previous.matchAll(/\S+/gu)].find(
      (match) =>
        match.index <= offset - edit.start && offset - edit.start < match.index + match[0].length,
    );
    if (token) {
      const start = edit.newText.indexOf(token[0]);
      if (start >= 0 && start === edit.newText.lastIndexOf(token[0])) {
        return edit.start + delta + start + offset - edit.start - token.index;
      }
    }
    return edit.start + delta + Math.min(offset - edit.start, edit.newText.length);
  }
  return offset + delta;
}
