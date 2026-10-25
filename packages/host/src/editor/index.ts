/**
 * Provides editor queries over a captured compiler graph and its design system.
 * Construct `EditorSnapshot` from the graph and the session and loader that compiled it.
 * Requests use the original document's UTF-16 offsets, and results identify that document's
 * revision so callers can reject stale ranges. Completion resolution also requires the
 * capture fingerprint returned with the completion.
 */

export type {
  EditorColor,
  EditorCompletion,
  EditorEdit,
  EditorRequest,
  EditorResult,
} from "./request.ts";
export { EditorSnapshot } from "./snapshot.ts";
