import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  TraceMap,
  decodedMappings,
  originalPositionFor,
  sourceContentFor,
} from "@jridgewell/trace-mapping";
import {
  fingerprint,
  type BindingLabel,
  type BindingLocation,
  type BindingReport,
} from "@stillwind/native";

import { cleanModuleId, type PreparedSource } from "../compilation/index.ts";

/**
 * Maps reports through upstream loader maps to their original source documents.
 *
 * @param reports - Reports with byte offsets and UTF-16 locations in prepared source documents.
 * @param sources - Prepared sources whose document metadata supplies upstream maps.
 * @param read - Reads original file text when the map has no embedded content, or when a label
 * needs a document-level fallback. Identities passed here are absolute file paths.
 * @returns Reports with original positions and content fingerprints. Unmappable labels become
 * document-scoped. An action is retained only when every edit maps to an unchanged text range.
 * @throws If required source reads or upstream map decoding fail.
 */
export async function mapAnalysisReports(
  reports: readonly BindingReport[],
  sources: readonly PreparedSource[],
  read: (id: string) => Promise<string>,
): Promise<BindingReport[]> {
  const documents = new Map(sources.map((source) => [source.document.uri, source]));

  const mapLocation = async (
    location: BindingLocation,
    exact: boolean,
  ): Promise<BindingLocation | undefined> => {
    const source = documents.get(location.uri);
    if (!source?.document.upstreamMap) {
      return location;
    }
    const trace = new TraceMap(
      source.document.upstreamMap,
      path.isAbsolute(cleanModuleId(source.resolverId))
        ? pathToFileURL(cleanModuleId(source.resolverId)).href
        : undefined,
    );
    const start = originalPositionFor(trace, {
      line: location.startLine + 1,
      column: location.startColumn,
    });
    const end = originalPositionFor(trace, {
      line: location.endLine + 1,
      column: location.endColumn,
    });
    const mappings = decodedMappings(trace);
    const endpoint = (line: number, column: number) =>
      mappings[line]?.some((segment) => segment.length > 1 && segment[0] === column);
    if (
      !endpoint(location.startLine, location.startColumn) ||
      !endpoint(location.endLine, location.endColumn)
    ) {
      return;
    }
    if (
      !start.source ||
      start.source !== end.source ||
      start.line === null ||
      end.line === null ||
      start.column === null ||
      end.column === null
    ) {
      return;
    }
    const uri = start.source.startsWith("file:")
      ? start.source
      : path.isAbsolute(start.source)
        ? pathToFileURL(start.source).href
        : undefined;
    if (!uri) {
      return;
    }
    const code = sourceContentFor(trace, start.source) ?? (await read(fileURLToPath(uri)));
    const offset = (line: number, column: number) => {
      const lines = code.split("\n");
      if (line < 1 || line > lines.length || column > lines[line - 1].replace(/\r$/, "").length) {
        return;
      }
      return lines.slice(0, line - 1).reduce((sum, text) => sum + text.length + 1, 0) + column;
    };
    const startUtf16 = offset(start.line, start.column);
    const endUtf16 = offset(end.line!, end.column!);
    if (
      startUtf16 === undefined ||
      endUtf16 === undefined ||
      endUtf16 < startUtf16 ||
      (exact &&
        code.slice(startUtf16, endUtf16) !==
          source.document.code.slice(location.startUtf16, location.endUtf16))
    ) {
      return;
    }
    return {
      uri,
      revision: fingerprint(code),
      start: Buffer.byteLength(code.slice(0, startUtf16)),
      end: Buffer.byteLength(code.slice(0, endUtf16)),
      startUtf16,
      endUtf16,
      startLine: start.line - 1,
      startColumn: start.column,
      endLine: end.line! - 1,
      endColumn: end.column!,
    };
  };

  const label = async (value: BindingLabel): Promise<BindingLabel> => {
    const mapped = await mapLocation(value.location, false);
    if (mapped) {
      return {
        ...value,
        location: mapped,
      };
    }
    const source = documents.get(value.location.uri);
    const file = source && cleanModuleId(source.resolverId);
    if (file && path.isAbsolute(file)) {
      const code = await read(file);
      return {
        ...value,
        scope: "document",
        location: {
          ...value.location,
          uri: pathToFileURL(file).href,
          revision: fingerprint(code),
          start: 0,
          end: 0,
          startUtf16: 0,
          endUtf16: 0,
          startLine: 0,
          endLine: 0,
          startColumn: 0,
          endColumn: 0,
        },
      };
    }
    return {
      ...value,
      scope: "document",
    };
  };

  return Promise.all(
    reports.map(async (report) => {
      const actions: BindingReport["actions"] = [];
      for (const action of report.actions) {
        const edits = await Promise.all(
          action.edits.map(async (edit) => {
            const location = await mapLocation(edit.location, true);
            return (
              location && {
                ...edit,
                location,
              }
            );
          }),
        );
        if (edits.every((edit) => edit !== undefined)) {
          actions.push({
            ...action,
            edits,
          });
        }
      }
      return {
        ...report,
        primary: await label(report.primary),
        related: await Promise.all(report.related.map(label)),
        actions,
      };
    }),
  );
}
