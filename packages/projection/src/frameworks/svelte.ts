import { parse } from "svelte/compiler";
import { svelte2tsx } from "svelte2tsx";
import ts from "typescript";

import type { SourceProjection } from "../protocol.ts";
import { converterIdentity, sourceMapV3, sourceRegions, type ScriptRegion } from "./common.ts";

export function convert(source: string, filename: string): SourceProjection {
  const ast = parse(source, {
    modern: true,
    filename,
  });
  const options = {
    isTsFile: true,
    emitOnTemplateError: false,
  } as const;
  const result = svelte2tsx(source, {
    ...options,
    filename,
  });
  const scripts: ScriptRegion[] = [];
  for (const [id, script] of [
    [0, ast.module],
    [1, ast.instance],
  ] as const) {
    if (!script) {
      continue;
    }
    const start = "start" in script.content ? script.content.start : undefined;
    const end = "end" in script.content ? script.content.end : undefined;
    if (typeof start !== "number" || typeof end !== "number") {
      throw new Error("Svelte script has no source coordinates");
    }
    scripts.push({
      id,
      start,
      end,
      ...(id === 1 && ast.module ? { parent: 0 } : {}),
    });
  }
  return {
    syntax: {
      language: "tsx",
      moduleKind: "module",
    },
    code: result.code,
    map: sourceMapV3(result.map),
    converter: converterIdentity("svelte2tsx", {
      ...options,
      svelte: converterIdentity("svelte", {}).version,
      typescript: ts.version,
    }),
    importRealms: scripts,
    editableRegions: sourceRegions(source, scripts, ast.instance ? 1 : ast.module ? 0 : undefined),
  };
}
