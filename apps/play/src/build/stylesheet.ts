/**
 * Replaces `@import` rules that name workspace stylesheets with their contents.
 *
 * @param files - Resolution and deferred reads from one captured workspace revision.
 * @param file - Absolute path of the stylesheet to read.
 * @param seen - Stylesheets already inlined on the current import path. A cycle leaves the rule.
 * @returns The stylesheet text. Imports of other URLs and media-qualified imports stay unchanged.
 */
export async function inlineImports(
  files: {
    read(file: string): Promise<string>;
    resolve(source: string, importer: string): string | undefined;
  },
  file: string,
  seen: Set<string>,
): Promise<string> {
  const text = await files.read(file);
  let output = "";
  let start = 0;
  for (const match of text.matchAll(/@import\s+(?:url\(\s*)?["']([^"']+)["']\s*\)?\s*;/g)) {
    output += text.slice(start, match.index);
    const target = files.resolve(match[1], file);
    output +=
      target?.endsWith(".css") && !seen.has(target)
        ? await inlineImports(files, target, new Set([...seen, file]))
        : match[0];
    start = match.index + match[0].length;
  }
  return output + text.slice(start);
}
