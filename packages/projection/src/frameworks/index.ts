import type { PreparedSource, ProjectInput, SourceProjection } from "../protocol.ts";

/**
 * Prepares an original document for direct parsing or conversion to a generated script.
 *
 * Optional converter dependencies are loaded only for the document's format. Input objects are
 * not mutated.
 *
 * @param input - Original document, host identities, and syntax for direct parsing. The projection
 * filename is passed to component converters, which supply syntax for their generated code.
 * @returns Parser input with the original text, URI, and upstream map retained. The document's
 * `sourceMapSource` defaults to its URI when omitted.
 * @throws If converter loading or conversion fails.
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
