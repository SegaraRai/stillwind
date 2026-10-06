import { PreparationError } from "./preparation.ts";
import type { PreparedSource, ProjectInput } from "./protocol.ts";

export type { CheckerBuffer, CheckerEnvironment } from "./checker.ts";
export { PreparationError } from "./preparation.ts";
export type * from "./protocol.ts";

/**
 * Prepares JS/TS input while retaining its document and compiler identities.
 * @param input - Original script or JSX document, syntax, and host metadata.
 * @returns Direct parser input with sourceMapSource defaulting to the document URI.
 * @throws PreparationError for component formats, which require caller-supplied projections.
 */
export async function prepareSource(input: ProjectInput): Promise<PreparedSource> {
  if (input.document.format !== "script" && input.document.format !== "jsx") {
    throw new PreparationError({
      kind: "projection",
      detail: "This browser host requires a prepared projection for component documents",
    });
  }
  return {
    ...input,
    kind: "direct",
    document: {
      ...input.document,
      format: input.document.format,
      sourceMapSource: input.document.sourceMapSource ?? input.document.uri,
    },
  };
}

/**
 * Returns browser checker requirements without filesystem reads.
 * @param _source - Direct or caller-prepared source. Its checker globals remain on its projection.
 * @returns No additional declaration paths. Embeddings include dependency declarations in their inventory.
 */
export function checkerEnvironment(_source: PreparedSource) {
  return {
    checkerTypes: [] as string[],
  };
}
