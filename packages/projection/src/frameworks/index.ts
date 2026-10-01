import type { ProjectSource } from "../protocol.ts";
import type { ConvertedFramework } from "./common.ts";

/**
 * Projects Vue, Svelte, or Astro input selected by sourceFormat, retaining its
 * original buffer in fwCode and moving its upstream map into fwMap.
 * The caller supplies the original format and projection filename. The returned copy
 * preserves framework and defaults sourceMapSource to uri for every input.
 * Already projected and other inputs retain their buffers without validation.
 * Rejects on converter loading or conversion errors; input is not mutated.
 */
export async function projectFrameworkSource(input: ProjectSource): Promise<ProjectSource> {
  if (input.fwCode !== undefined) {
    return {
      ...input,
      sourceMapSource: input.sourceMapSource ?? input.uri,
    };
  }
  let converted: ConvertedFramework;
  // Load optional converter dependencies only for their source format.
  switch (input.sourceFormat) {
    case "astro":
      converted = await (
        await import("./astro.ts")
      ).convert(input.source, input.projectionFileName);
      break;
    case "svelte":
      converted = (await import("./svelte.ts")).convert(input.source, input.projectionFileName);
      break;
    case "vue":
      converted = (await import("./vue.ts")).convert(input.source, input.projectionFileName);
      break;
    default:
      return {
        ...input,
        sourceMapSource: input.sourceMapSource ?? input.uri,
      };
  }
  return {
    ...input,
    ...converted,
    sourceMapSource: input.sourceMapSource ?? input.uri,
    fwCode: input.source,
    fwMap: input.map,
    map: undefined,
  };
}
