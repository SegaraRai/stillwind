import { parse } from "svelte/compiler";
import { svelte2tsx } from "svelte2tsx";
import ts from "typescript";

import {
  converterIdentity,
  sourceMapV3,
  sourceRegions,
  type ConvertedFramework,
  type ScriptRegion,
} from "./index.ts";

export function convert(source: string, filename: string): ConvertedFramework {
  const ast = parse(source, { modern: true, filename });
  const options = { isTsFile: true, emitOnTemplateError: false } as const;
  const result = svelte2tsx(source, { ...options, filename });
  const scripts: ScriptRegion[] = [];
  for (const [id, script] of [
    [0, ast.module],
    [1, ast.instance],
  ] as const) {
    if (!script) continue;
    const start = "start" in script.content ? script.content.start : undefined;
    const end = "end" in script.content ? script.content.end : undefined;
    if (typeof start !== "number" || typeof end !== "number")
      throw new Error("Svelte script has no source coordinates");
    scripts.push({ id, start, end });
  }
  return {
    framework: "Svelte",
    language: "tsx",
    source: result.code,
    fw2js_map: sourceMapV3(result.map),
    converter: converterIdentity("svelte2tsx", {
      ...options,
      svelte: converterIdentity("svelte", {}).version,
      typescript: ts.version,
    }),
    import_realms: scripts,
    regions: sourceRegions(source, scripts, ast.instance ? 1 : ast.module ? 0 : undefined),
  };
}
