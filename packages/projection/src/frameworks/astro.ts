import { convertToTSX, parse } from "@astrojs/compiler";

import {
  converterIdentity,
  sourceMapV3,
  sourceRegions,
  type ConvertedFramework,
  type ScriptRegion,
} from "./common.ts";

export async function convert(source: string, filename: string): Promise<ConvertedFramework> {
  const options = { includeScripts: true };
  const [result, parsed] = await Promise.all([
    convertToTSX(source, { ...options, filename }),
    parse(source, { position: true }),
  ]);
  const errors = [...result.diagnostics, ...parsed.diagnostics].filter(
    (diagnostic) => diagnostic.severity === 1,
  );
  if (errors.length) {
    throw new Error(errors.map((diagnostic) => diagnostic.text).join("\n"));
  }
  const scripts: ScriptRegion[] = [];
  const frontmatter = parsed.ast.children.find((node) => node.type === "frontmatter");
  if (frontmatter?.type === "frontmatter" && frontmatter.position?.end) {
    // Astro AST points are UTF-8 byte offsets; TSX extracted script locations are UTF-16.
    const bytes = Buffer.from(source);
    const end = bytes.subarray(0, frontmatter.position.end.offset).toString("utf8").length - 3;
    const start = end - frontmatter.value.length;
    if (source.slice(start, end) !== frontmatter.value || source.slice(end, end + 3) !== "---") {
      throw new Error("Astro frontmatter coordinates do not match source");
    }
    scripts.push({ id: 0, start, end });
  }
  for (const script of result.metaRanges.scripts ?? []) {
    if (!["processed-module", "module", "inline"].includes(script.type)) {
      continue;
    }
    const { start, end } = script.position;
    if (source.slice(start, end) !== script.content) {
      throw new Error("Astro client script coordinates do not match source");
    }
    scripts.push({ id: scripts.length + 1, start, end });
  }
  return {
    checkerScripts: (result.metaRanges.scripts ?? [])
      .filter(
        (script) =>
          ["processed-module", "module", "inline"].includes(script.type) &&
          script.content.length > 0,
      )
      .map((script) => ({ start: script.position.start, end: script.position.end })),
    sourceFormat: "astro",
    language: "tsx",
    source: result.code,
    fw2JsMap: sourceMapV3(result.map),
    converter: converterIdentity("@astrojs/compiler", options),
    importRealms: scripts,
    regions: sourceRegions(source, scripts, frontmatter ? 0 : undefined),
  };
}
