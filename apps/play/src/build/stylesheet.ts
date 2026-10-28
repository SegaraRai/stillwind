import { resolveVirtualModule } from "@stillwind/host/resolution";

/**
 * Replaces `@import` rules that name workspace stylesheets with their contents.
 * @param files - Virtual workspace files keyed by absolute path.
 * @param file - Absolute path of the stylesheet to read.
 * @param seen - Stylesheets already inlined on the current import path. A cycle leaves the rule.
 * @returns The stylesheet text. Imports of other URLs and media-qualified imports stay unchanged.
 */
export function inlineImports(
  files: ReadonlyMap<string, string>,
  file: string,
  seen: Set<string>,
): string {
  const text = files.get(file) ?? "";
  return text.replace(
    /@import\s+(?:url\(\s*)?["']([^"']+)["']\s*\)?\s*;/g,
    (rule, source: string) => {
      const target = resolveVirtualModule(files, source, file);
      if (!target?.endsWith(".css") || seen.has(target)) {
        return rule;
      }
      return inlineImports(files, target, new Set([...seen, file]));
    },
  );
}
