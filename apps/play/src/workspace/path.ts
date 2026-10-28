/**
 * Normalizes a path inside the workspace. Backslashes become slashes, and `.` and `..` segments
 * are resolved. Leading slashes and empty segments are removed, so the path is always relative
 * to the workspace root.
 *
 * @param path - Path relative to the workspace root, as a user or a permalink supplies it.
 * @returns The normalized relative path, such as `components/Counter.tsx`, or `null` when the
 *   path is empty or a `..` segment leaves the workspace.
 */
export function normalizeWorkspacePath(path: string): string | null {
  const segments: string[] = [];
  for (const segment of path.replaceAll("\\", "/").split("/")) {
    if (segment === "" || segment === ".") {
      continue;
    }
    if (segment === "..") {
      if (segments.length === 0) {
        return null;
      }
      segments.pop();
      continue;
    }
    segments.push(segment);
  }
  return segments.length > 0 ? segments.join("/") : null;
}
