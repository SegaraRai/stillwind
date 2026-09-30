import type { ProjectSource } from "../protocol.ts";
import type { ConvertedFramework } from "./common.ts";

/**
 * Projects Vue, Svelte, or Astro input selected by source_format, retaining its
 * original buffer in fw_code and moving its upstream map into fw_map.
 * The caller supplies the original format and projection filename. The returned copy
 * preserves framework and defaults source_map_source to uri for every input.
 * Already projected and other inputs retain their buffers without validation.
 * Rejects on converter loading or conversion errors; input is not mutated.
 */
export async function projectFrameworkSource(input: ProjectSource): Promise<ProjectSource> {
  if (input.fw_code !== undefined) {
    return {
      ...input,
      source_map_source: input.source_map_source ?? input.uri,
    };
  }
  let converted: ConvertedFramework;
  // Load optional converter dependencies only for their source format.
  switch (input.source_format) {
    case "astro":
      converted = await (
        await import("./astro.ts")
      ).convert(input.source, input.projection_file_name);
      break;
    case "svelte":
      converted = (await import("./svelte.ts")).convert(input.source, input.projection_file_name);
      break;
    case "vue":
      converted = (await import("./vue.ts")).convert(input.source, input.projection_file_name);
      break;
    default:
      return {
        ...input,
        source_map_source: input.source_map_source ?? input.uri,
      };
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
