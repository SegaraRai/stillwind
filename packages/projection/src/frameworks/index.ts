import { extname } from "node:path";

import type { ProjectSource } from "../protocol.ts";
import type { ConvertedFramework } from "./common.ts";

function scriptLanguage(path: string): ProjectSource["language"] {
  const name = path.replace(/[?#].*$/, "").toLowerCase();
  if (name.endsWith(".d.ts") || name.endsWith(".d.cts") || name.endsWith(".d.mts")) {
    return "dts";
  }
  if (name.endsWith(".tsx")) {
    return "tsx";
  }
  if (name.endsWith(".ts") || name.endsWith(".cts") || name.endsWith(".mts")) {
    return "ts";
  }
  if (name.endsWith(".jsx")) {
    return "jsx";
  }
  return "js";
}

/**
 * Projects Vue, Svelte, or Astro input selected by path extension, retaining its
 * original buffer in fw_code and moving its upstream map into fw_map.
 * Already projected and other inputs are returned unchanged without validation.
 * Rejects on converter loading/conversion errors or a conflicting framework hint.
 */
export async function projectFrameworkSource(
  input: Omit<ProjectSource, "language" | "module_kind"> &
    Partial<Pick<ProjectSource, "language" | "module_kind">>,
): Promise<ProjectSource> {
  if (input.fw_code !== undefined) {
    return {
      ...input,
      language: input.language ?? scriptLanguage(input.path),
      module_kind: input.module_kind ?? "unambiguous",
      source_map_source: input.source_map_source ?? input.uri,
    };
  }
  let converted: ConvertedFramework;
  // We have to use dynamic import since framework specific file imports an optional dependency.
  switch (extname(input.path).toLowerCase()) {
    case ".astro":
      converted = await (await import("./astro.ts")).convert(input.source, input.path);
      break;
    case ".svelte":
      converted = (await import("./svelte.ts")).convert(input.source, input.path);
      break;
    case ".vue":
      converted = (await import("./vue.ts")).convert(input.source, input.path);
      break;
    default:
      return {
        ...input,
        language: input.language ?? scriptLanguage(input.path),
        module_kind: input.module_kind ?? "unambiguous",
        source_map_source: input.source_map_source ?? input.uri,
      };
  }
  if (input.framework !== undefined && input.framework !== converted.framework) {
    throw new Error(
      `Framework ${input.framework} conflicts with ${converted.framework} source ${input.path}`,
    );
  }
  return {
    ...input,
    ...converted,
    module_kind: input.module_kind ?? "unambiguous",
    source_map_source: input.source_map_source ?? input.uri,
    fw_code: input.source,
    fw_map: input.map,
    map: undefined,
  };
}
