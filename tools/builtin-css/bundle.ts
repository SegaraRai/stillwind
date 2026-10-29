import { readFile } from "node:fs/promises";
import path from "node:path";

import { parse } from "css-tree";

/**
 * Expands local imports in source order and preserves each import's named layer.
 */
export async function bundleCss(filename: string, ancestors: string[] = []): Promise<string> {
  filename = path.resolve(filename);
  if (ancestors.includes(filename)) {
    throw new Error(`Circular CSS import: ${[...ancestors, filename].join(" -> ")}`);
  }
  const source = (await readFile(filename, "utf8")).replace(/\r\n/g, "\n");
  const ast = parse(source, {
    positions: true,
    parseAtrulePrelude: false,
    parseRulePrelude: false,
    parseValue: false,
  });
  if (ast.type !== "StyleSheet") {
    throw new Error(`Expected a stylesheet: ${filename}`);
  }
  let output = "";
  let cursor = 0;
  for (const node of ast.children) {
    if (node.type !== "Atrule" || node.name.toLowerCase() !== "import") {
      continue;
    }
    const prelude = node.prelude;
    const match =
      prelude?.type === "Raw"
        ? /^(["'])(\.\.?\/[^"']+)\1(?:\s+layer\(([\w.-]+)\))?$/.exec(prelude.value.trim())
        : null;
    if (!match || !node.loc) {
      throw new Error(
        `Unsupported CSS import in ${filename}: ${source.slice(node.loc?.start.offset, node.loc?.end.offset)}`,
      );
    }
    const body = await bundleCss(path.resolve(path.dirname(filename), match[2]), [
      ...ancestors,
      filename,
    ]);
    output += source.slice(cursor, node.loc.start.offset);
    output += match[3] ? `@layer ${match[3]} {\n${body}\n}` : body;
    cursor = node.loc.end.offset;
  }
  return output + source.slice(cursor);
}
