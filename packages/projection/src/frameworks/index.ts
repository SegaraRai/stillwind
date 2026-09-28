import type { ProjectSource } from "../protocol.ts";
import type { ConvertedFramework } from "./common.ts";

/**
 * Projects Vue, Svelte, or Astro input selected by the supplied framework, retaining its
 * original buffer in fw_code and moving its upstream map into fw_map.
 * Already projected and other inputs are returned unchanged without validation.
 * Rejects on converter loading/conversion errors or a conflicting framework hint.
 */
export async function projectFrameworkSource(input: ProjectSource): Promise<ProjectSource> {
  if (input.fw_code !== undefined) {
    return {
      ...input,
      source_map_source: input.source_map_source ?? input.uri,
    };
  }
  let converted: ConvertedFramework;
  // We have to use dynamic import since framework specific file imports an optional dependency.
  switch (input.framework) {
    case "Astro":
      converted = await (
        await import("./astro.ts")
      ).convert(input.source, input.projection_file_name);
      break;
    case "Svelte":
      converted = (await import("./svelte.ts")).convert(input.source, input.projection_file_name);
      break;
    case "Vue":
      converted = (await import("./vue.ts")).convert(input.source, input.projection_file_name);
      break;
    default:
      return {
        ...input,
        source_map_source: input.source_map_source ?? input.uri,
      };
  }
  if (input.framework !== undefined && input.framework !== converted.framework) {
    throw new Error(
      `Framework ${input.framework} conflicts with ${converted.framework} source ${input.resolver_id}`,
    );
  }
  return {
    ...input,
    ...converted,
    source_map_source: input.source_map_source ?? input.uri,
    fw_code: input.source,
    fw_map: input.map,
    map: undefined,
  };
}
