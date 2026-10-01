import type { PreparedSource, ProjectInput, SourceProjection } from "../protocol.ts";

/**
 * Prepares the original document using its required format and projection filename.
 * Direct documents retain the supplied syntax; component converters supply syntax
 * for generated code. The returned document preserves original code, URI, and upstream
 * mapping, and defaults sourceMapSource to its URI. Input objects are not mutated.
 * Rejects if converter loading or conversion fails.
 */
export async function prepareSource(input: ProjectInput): Promise<PreparedSource> {
  const { document: original, syntax, ...metadata } = input;
  const document = {
    ...original,
    sourceMapSource: original.sourceMapSource ?? original.uri,
  };
  let projection: SourceProjection;
  // Load optional converter dependencies only for their document format.
  switch (document.format) {
    case "astro":
      projection = await (
        await import("./astro.ts")
      ).convert(document.code, input.projectionFileName);
      break;
    case "svelte":
      projection = (await import("./svelte.ts")).convert(document.code, input.projectionFileName);
      break;
    case "vue":
      projection = (await import("./vue.ts")).convert(document.code, input.projectionFileName);
      break;
    case "script":
    case "jsx":
      return {
        ...metadata,
        kind: "direct",
        document: {
          ...document,
          format: document.format,
        },
        syntax,
      };
  }
  return {
    ...metadata,
    kind: "projected",
    document: {
      ...document,
      format: document.format,
    },
    projection,
  };
}
