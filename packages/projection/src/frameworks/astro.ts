import { convertToTsx, type ConvertToTsxResult } from "@astrojs/astro2tsx";

import { PreparationError, type ScriptRegion } from "../preparation.ts";
import type { ConverterIdentity, ProjectionMap, SourceProjection } from "../protocol.ts";
import { converterIdentity, sourceRegions } from "./common.ts";

/** Script types whose bodies are JavaScript or TypeScript that Stillwind scans and checks. */
const clientScriptTypes = new Set(["processed-module", "module", "inline"]);

/** Text that opens and closes each client script body inserted into an empty script element. */
const clientWrapper = ["{() => {", "}}"] as const;

/**
 * Converts an Astro component to TSX and records original scopes for compiler edits and checking.
 *
 * `@astrojs/astro2tsx` leaves script elements empty. Each client script body is inserted into
 * its empty element as `{() => {body}}`, so macro calls in client scripts appear in the TSX.
 *
 * @param source - Complete original Astro component text.
 * @param filename - Component filename passed to the converter to name the generated component.
 * @returns Generated TSX with Volar mappings, frontmatter and client script realms, and nonempty
 * client script ranges for independent checking. Template edits require a frontmatter realm.
 * @throws {PreparationError} If Astro reports a syntax error, script coordinates disagree with the
 * original text, or a client script cannot be placed in the generated TSX.
 * @throws If converter metadata cannot be loaded or conversion otherwise fails.
 */
export async function convert(source: string, filename: string): Promise<SourceProjection> {
  const options = {
    clientScripts: "arrow-function",
  };
  const converter = converterIdentity("@astrojs/astro2tsx", options);

  const result = convertToTsx(source, {
    filename,
  });
  const error = result.diagnostics.find((diagnostic) => diagnostic.severity === 1);
  if (error || result.hasParseErrors || result.frontmatterStatus === "open") {
    throw new PreparationError(
      {
        kind: "syntax",
        detail: error?.message ?? "Astro reported a syntax error",
      },
      converter,
    );
  }

  const scripts: ScriptRegion[] = [];

  let frontmatter: number | undefined;
  if (result.frontmatterStatus === "closed") {
    // `frontmatterSource` includes both `---` fences.
    const start = result.frontmatterSource.start + 3;
    const end = result.frontmatterSource.end - 3;
    if (
      start > end ||
      source.slice(start - 3, start) !== "---" ||
      source.slice(end, end + 3) !== "---"
    ) {
      throw projectionError("Astro frontmatter coordinates do not match source", converter);
    }
    frontmatter = 0;
    scripts.push({
      id: 0,
      start,
      end,
    });
  }

  const clients = result.scripts.filter((script) => clientScriptTypes.has(script.type));
  for (const script of clients) {
    const { start, end } = script.position;
    if (source.slice(start, end) !== script.content) {
      throw projectionError("Astro client script coordinates do not match source", converter);
    }
    scripts.push({
      id: scripts.length + 1,
      start,
      end,
    });
  }

  const { code, map } = insertClientScripts(result, clients, converter);
  return {
    checkerScriptRanges: clients
      .filter((script) => script.content.length > 0)
      .map((script) => ({
        start: script.position.start,
        end: script.position.end,
      })),
    syntax: {
      language: "tsx",
      moduleKind: "module",
    },
    code,
    map,
    converter,
    importRealms: scripts,
    editableRegions: sourceRegions(source, scripts, frontmatter, converter),
  };
}

/**
 * Inserts client script bodies into the empty script elements of converted TSX.
 *
 * @param result - Converter output whose source mappings use UTF-16 offsets.
 * @param clients - Nonoverlapping client scripts in source order whose content matches the
 * original text at their positions.
 * @param converter - Converter identity attached to a projection failure.
 * @returns TSX with each nonempty body wrapped in `{() => {` and `}}`, and Volar mappings for the
 * copied source spans and the inserted bodies.
 * @throws {PreparationError} If no unique generated position follows the opening tag of a
 * nonempty script.
 */
function insertClientScripts(
  result: ConvertToTsxResult,
  clients: ConvertToTsxResult["scripts"],
  converter: ConverterIdentity,
): {
  code: string;
  map: ProjectionMap;
} {
  // Generator-only spans have no original text. Their anchors would not identify source edits.
  const copied = result.mappings.filter(
    ([, , , originalLength, kind]) => kind === 0 && originalLength > 0,
  );
  // An empty body has no text to map, and an empty element is already valid TSX.
  const insertions = clients
    .filter(({ content }) => content)
    .map((script) => {
      const at = new Set(
        copied
          .filter(
            ([, , originalStart, originalLength]) =>
              originalStart + originalLength === script.position.start,
          )
          .map(([generatedStart, generatedLength]) => generatedStart + generatedLength),
      );
      if (at.size !== 1) {
        throw projectionError("Cannot place the Astro client script in the TSX", converter);
      }
      return {
        at: [...at][0],
        script,
      };
    });

  const sourceOffsets: number[] = [];
  const generatedOffsets: number[] = [];
  const lengths: number[] = [];
  const generatedLengths: number[] = [];
  /**
   * Shifts an offset in converter output past the bodies inserted at or before it.
   *
   * @param offset - UTF-16 offset in the converter output.
   * @returns The corresponding UTF-16 offset in the returned code.
   */
  const shift = (offset: number) =>
    offset +
    insertions
      .filter(({ at }) => at <= offset)
      .reduce(
        (total, { script }) =>
          total + clientWrapper[0].length + script.content.length + clientWrapper[1].length,
        0,
      );
  for (const [generatedStart, generatedLength, originalStart, originalLength] of copied) {
    sourceOffsets.push(originalStart);
    generatedOffsets.push(shift(generatedStart));
    lengths.push(originalLength);
    generatedLengths.push(generatedLength);
  }

  let code = "";
  let cursor = 0;
  for (const { at, script } of [...insertions].sort((a, b) => a.at - b.at)) {
    code += result.code.slice(cursor, at) + clientWrapper[0];
    sourceOffsets.push(script.position.start);
    generatedOffsets.push(code.length);
    lengths.push(script.content.length);
    generatedLengths.push(script.content.length);
    code += script.content + clientWrapper[1];
    cursor = at;
  }
  code += result.code.slice(cursor);

  return {
    code,
    map: {
      kind: "volar",
      mappings: [
        {
          sourceOffsets,
          generatedOffsets,
          lengths,
          generatedLengths,
        },
      ],
    },
  };
}

/**
 * Creates a projection failure for inconsistent converter output.
 *
 * @param detail - Message that describes the inconsistency.
 * @param converter - Converter identity attached to the failure.
 * @returns The failure to throw.
 */
function projectionError(detail: string, converter: ConverterIdentity) {
  return new PreparationError(
    {
      kind: "projection",
      detail,
    },
    converter,
  );
}
