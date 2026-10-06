import { parse, type AST } from "svelte/compiler";
import { svelte2tsx, type SvelteCompiledToTsx } from "svelte2tsx";
import ts from "typescript";

import type { SourceProjection } from "../protocol.ts";
import {
  converterIdentity,
  originalRange,
  PreparationError,
  sourceMapV3,
  sourceRegions,
  type ScriptRegion,
} from "./common.ts";

/**
 * Converts a Svelte component to TypeScript and records the original scopes for compiler edits.
 *
 * @param source - Complete original Svelte component text.
 * @param filename - Component filename passed to the parser and converter for diagnostics and maps.
 * @returns Generated TypeScript and its version 3 map. The instance realm inherits module imports
 * when both scripts exist. Template edits use the instance realm, or the module realm when only
 * that script exists. Components without either script have no editable template ranges.
 * @throws {PreparationError} If the compiler reports a recognized syntax error, script coordinates
 * are missing or invalid, or the projection map cannot be used.
 * @throws If converter metadata cannot be loaded or conversion otherwise fails.
 */
export function convert(source: string, filename: string): SourceProjection {
  const options = {
    isTsFile: true,
    emitOnTemplateError: false,
  } as const;
  const converter = converterIdentity("svelte2tsx", {
    ...options,
    svelte: converterIdentity("svelte", {}).version,
    typescript: ts.version,
  });

  let ast: AST.Root;
  let result: SvelteCompiledToTsx;
  try {
    ast = parse(source, {
      modern: true,
      filename,
    });
    result = svelte2tsx(source, {
      ...options,
      filename,
    });
  } catch (error) {
    if (!(error instanceof Error) || error.name !== "CompileError" || !("code" in error)) {
      throw error;
    }
    const diagnostic = error as Error & {
      start?: { character: number };
      end?: { character: number };
    };
    throw new PreparationError(
      {
        kind: "syntax",
        detail: error.message,
      },
      converter,
      originalRange(source, diagnostic.start?.character, diagnostic.end?.character),
    );
  }

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
      throw new PreparationError(
        {
          kind: "projection",
          detail: "Svelte script has no source coordinates",
        },
        converter,
      );
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
      language: "ts",
      moduleKind: "module",
    },
    code: result.code,
    map: sourceMapV3(result.map, converter),
    converter,
    importRealms: scripts,
    editableRegions: sourceRegions(
      source,
      scripts,
      ast.instance ? 1 : ast.module ? 0 : undefined,
      converter,
    ),
  };
}
