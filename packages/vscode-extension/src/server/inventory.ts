import { virtualPath } from "@stillwind/host/browser";

/**
 * Moves captured files into the workspace inventory and releases the capture's file list.
 *
 * @param captured - Owned capture with original document URI/text pairs. Its files become empty.
 * @returns File text indexed by decoded virtual paths, independent of later capture changes.
 * @throws If a document URI or its percent encoding is invalid.
 */
export function consumeWorkspaceFiles(captured: {
  files: readonly (readonly [string, string])[];
}): Map<string, string> {
  const files = new Map<string, string>();
  for (const [uri, text] of captured.files) {
    files.set(virtualPath(uri), text);
  }
  captured.files = [];
  return files;
}
