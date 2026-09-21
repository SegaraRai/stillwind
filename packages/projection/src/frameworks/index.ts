import { extname } from "node:path";

import type { ProjectSource } from "../protocol.ts";
import type { ConvertedFramework } from "./common.ts";

/**
 * Projects Vue, Svelte, or Astro input selected by path extension, retaining its
 * original buffer in fw_code and moving its upstream map into fw_map.
 * Already projected and other inputs are returned unchanged without validation.
 * Rejects on converter loading/conversion errors or a conflicting framework hint.
 */
export async function projectFrameworkSource(input: ProjectSource): Promise<ProjectSource> {
  if (input.fw_code !== undefined) {
    return input;
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
      return input;
  }
  if (input.framework !== undefined && input.framework !== converted.framework) {
    throw new Error(
      `Framework ${input.framework} conflicts with ${converted.framework} source ${input.path}`,
    );
  }
  return {
    ...input,
    ...converted,
    fw_code: input.source,
    fw_map: input.map,
    map: undefined,
  };
}
