import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  isStylesheet,
  type EditorRequest,
  type EditorResult,
  type WorkspaceResult,
} from "@stillwind/host";

import { lintReports } from "./reports.ts";
import { resolveRewriteConfig, type RewriteOptions } from "./rewrite-files.ts";
import { AnalysisWorker } from "./worker-client.ts";
import type { WorkspaceEditorResult } from "./workspace.ts";

/**
 * Selects generated CSS by file, line, or position.
 */
export interface InspectTarget {
  /** Path or glob accepted by target selection. */
  pattern: string;
  /** One-based line. Omission selects the whole file. */
  line?: number;
  /** One-based UTF-16 column. It requires line. */
  column?: number;
}

/**
 * Generated CSS for one source range in one project.
 */
export interface InspectPreview {
  /** Zero-based project index in the evaluated configuration. */
  project: number;
  /** Original document range with zero-based lines and UTF-16 columns, and an exclusive end. */
  location: {
    uri: string;
    startLine: number;
    startColumn: number;
    endLine: number;
    endColumn: number;
  };
  /** Original source text in the range. */
  source: string;
  /** Formatted CSS. It is empty for a failed occurrence or a marker class. */
  css: string;
  /** False when diagnostics contains an error, or when a required style failed. */
  valid: boolean;
  /** Reports whose primary range overlaps this range. CSS omits the utilities that they reject. */
  diagnostics: {
    /** Report severity, as in the reports of lint. */
    severity: string;
    /** Stable compiler code or lint rule. */
    code: string;
    /** Description of the finding. */
    message: string;
  }[];
  /** Markdown for a marker class. */
  documentation?: string;
  /** Authored runtime key expressions that the CSS depends on. */
  runtimeNames?: string[];
}

/**
 * An editor answer with a source range. Hovers omit valid because they describe generated CSS.
 */
type PreviewRange = NonNullable<EditorResult["hover"]> & {
  valid?: boolean;
};

/**
 * A target that could not be read.
 */
export interface InspectFailure {
  /** Absolute path of the target. */
  file: string;
  /** Reason for the failure. */
  message: string;
}

/**
 * Inspection results with analysis outcomes retained for each project.
 */
export interface InspectResult {
  /** Absolute path of the evaluated configuration. */
  config: string;
  /** Analysis of each selected file, in file and project order. */
  results: WorkspaceResult[];
  /** Selected previews in file, project, and source order, without repeated ranges. */
  previews: InspectPreview[];
  /** Targets that could not be read. */
  failures: InspectFailure[];
}

/**
 * Parses a command-line target with an optional `:line` or `:line:column` suffix.
 *
 * @param target - Path or glob, including absolute Windows paths.
 * @returns The pattern and any one-based position.
 * @throws If the position is not positive or the target names a stylesheet.
 */
export function parseInspectTarget(target: string): InspectTarget {
  const [, pattern, line, column] = /^(.+?)(?::(\d+)(?::(\d+))?)?$/s.exec(target) ?? [];
  if (!pattern || /^0+$/.test(line ?? "") || /^0+$/.test(column ?? "")) {
    throw new Error(`Invalid inspect target: ${target}`);
  }
  if (isStylesheet(pattern)) {
    throw new Error(`inspect does not accept stylesheets: ${target}`);
  }
  return {
    pattern,
    ...(line ? { line: Number(line) } : {}),
    ...(column ? { column: Number(column) } : {}),
  };
}

/**
 * Finds the UTF-16 offset of the start of each line.
 *
 * @param source - Document text. Lines end at `\n`, so `\r\n` remains one line break.
 * @returns Ascending offsets, beginning with zero.
 */
function lineOffsets(source: string): number[] {
  const offsets = [0];
  for (let index = source.indexOf("\n"); index >= 0; index = source.indexOf("\n", index + 1)) {
    offsets.push(index + 1);
  }
  return offsets;
}

/**
 * Converts an offset to a position.
 *
 * @param lines - Result of {@link lineOffsets}.
 * @param offset - UTF-16 offset in the same document.
 * @returns The zero-based line and column.
 */
function position(lines: readonly number[], offset: number) {
  let line = lines.length - 1;
  while (lines[line] > offset) {
    line--;
  }
  return {
    line,
    column: offset - lines[line],
  };
}

/**
 * Checks whether a report location belongs to a file.
 *
 * @param uri - File URI or path.
 * @param file - Absolute path.
 * @returns Whether both name the same path.
 */
function isDocument(uri: string, file: string): boolean {
  return path.resolve(uri.startsWith("file:") ? fileURLToPath(uri) : uri) === path.resolve(file);
}

/**
 * Answers one target for one file.
 *
 * @param worker - Opened worker that owns the file's analysis.
 * @param file - Absolute target path.
 * @param source - Text of the file.
 * @param target - Position to select. Positions outside the file select nothing.
 * @param reports - Reports with a range in this file, for each project.
 * @returns Previews with their project, in project and source order.
 * @throws If the editor query fails.
 */
async function selectPreviews(
  worker: AnalysisWorker,
  file: string,
  source: string,
  target: InspectTarget,
  reports: ReturnType<typeof lintReports>,
): Promise<InspectPreview[]> {
  const lines = lineOffsets(source);
  const line = target.line === undefined ? undefined : target.line - 1;
  const lineStart = line === undefined ? undefined : lines[line];
  const lineEnd = line === undefined ? undefined : (lines[line + 1] ?? source.length + 1) - 1;
  if (line !== undefined && lineStart === undefined) {
    return [];
  }
  const offset = target.column === undefined ? undefined : lineStart! + target.column - 1;
  if (offset !== undefined && offset > lineEnd!) {
    return [];
  }
  const query = (request: EditorRequest) =>
    worker.request<WorkspaceEditorResult[]>({ method: "query", file, request });
  const uri = pathToFileURL(file).href;
  let selected: { project: number; range: PreviewRange }[];
  if (offset === undefined) {
    selected = (
      await query({
        kind: "previews",
      })
    ).flatMap(({ project, result }) =>
      (result.previews ?? [])
        .filter(({ start, end }) => line === undefined || (end > lineStart! && start <= lineEnd!))
        .map((range) => ({
          project,
          range,
        })),
    );
  } else {
    const hovers = await query({
      kind: "hover",
      offset,
    });
    selected = hovers.flatMap(({ project, result }) =>
      result.hover ? [{ project, range: result.hover }] : [],
    );
    if (selected.length < hovers.length) {
      // Failed occurrences and Vue template expressions have no hover, so the listing supplies them.
      const answered = new Set(selected.map(({ project }) => project));
      selected.push(
        ...(
          await query({
            kind: "previews",
          })
        ).flatMap(({ project, result }) =>
          answered.has(project)
            ? []
            : (result.previews ?? [])
                .filter(({ start, end }) => start <= offset && offset <= end)
                .map((range) => ({
                  project,
                  range,
                })),
        ),
      );
    }
  }
  return selected.flatMap(
    ({ project, range: { start, end, css, valid = true, documentation, runtimeNames } }) => {
      const diagnostics = reports
        .filter(
          ({ project: owner, report: { primary } }) =>
            owner === project &&
            primary.location.startUtf16 < end &&
            start < Math.max(primary.location.endUtf16, primary.location.startUtf16 + 1),
        )
        .map(({ report: { severity, code, message } }) => ({ severity, code, message }));
      if (!css && valid && !diagnostics.length && !documentation) {
        return [];
      }
      const first = position(lines, start);
      const last = position(lines, end);
      return [
        {
          project,
          location: {
            uri,
            startLine: first.line,
            startColumn: first.column,
            endLine: last.line,
            endColumn: last.column,
          },
          source: source.slice(start, end),
          css,
          valid: valid && !diagnostics.some(({ severity }) => severity === "error"),
          diagnostics,
          ...(documentation ? { documentation } : {}),
          ...(runtimeNames ? { runtimeNames } : {}),
        },
      ];
    },
  );
}

/**
 * Reports the generated CSS of configured source files in a child process.
 *
 * @param options - Configuration discovery and targets. Paths and globs follow the target rules
 * of lint. Stylesheets are not accepted, and an empty list selects every configured source file.
 * @returns Previews and analysis results. A file with incomplete analysis has no previews.
 * @throws If a target is invalid or matches no file, or if configuration discovery, worker
 * initialization, target enumeration, or cleanup fails.
 */
export async function inspect(
  options: Omit<RewriteOptions, "patterns"> & {
    /** Targets as written on the command line. */
    targets?: readonly string[];
  } = {},
): Promise<InspectResult> {
  const targets = (options.targets ?? []).map(parseInspectTarget);
  const config = resolveRewriteConfig(options);
  const worker = new AnalysisWorker();
  try {
    await worker.open(config);
    const selections = new Map<string, InspectTarget[]>();
    for (const target of targets.length ? targets : [undefined]) {
      const files = (await worker.targets(target ? [target.pattern] : undefined)).filter(
        (file) => !isStylesheet(file),
      );
      if (!files.length) {
        throw new Error(
          target
            ? `No analysis targets matched ${target.pattern}`
            : "No analysis targets matched the configured scopes",
        );
      }
      for (const file of files) {
        selections.set(file, [
          ...(selections.get(file) ?? []),
          target ?? {
            pattern: file,
          },
        ]);
      }
    }
    const files = [...selections.keys()].sort();
    await worker.request({
      method: "prepare",
      files,
    });

    const result: InspectResult = {
      config,
      results: [],
      previews: [],
      failures: [],
    };
    for (const file of files) {
      const analyses = await worker.analyze(file);
      result.results.push(...analyses);
      const complete = new Set(
        analyses.filter(({ result }) => result.status === "complete").map(({ project }) => project),
      );
      let source: string;
      try {
        const bytes = await readFile(file);
        source = bytes.toString("utf8");
        if (!Buffer.from(source).equals(bytes)) {
          throw new Error(`Source is not valid UTF-8: ${file}`);
        }
      } catch (error) {
        result.failures.push({
          file,
          message: error instanceof Error ? error.message : String(error),
        });
        continue;
      }
      const reports = lintReports(analyses).filter(
        ({ report }) =>
          report.primary.scope === "range" && isDocument(report.primary.location.uri, file),
      );
      const seen = new Set<string>();
      const previews: InspectPreview[] = [];
      for (const target of selections.get(file)!) {
        for (const preview of await selectPreviews(worker, file, source, target, reports)) {
          const { startLine, startColumn, endLine, endColumn } = preview.location;
          const key = [preview.project, startLine, startColumn, endLine, endColumn].join();
          if (complete.has(preview.project) && !seen.has(key)) {
            seen.add(key);
            previews.push(preview);
          }
        }
      }
      result.previews.push(
        ...previews.sort(
          (a, b) =>
            a.project - b.project ||
            a.location.startLine - b.location.startLine ||
            a.location.startColumn - b.location.startColumn,
        ),
      );
    }
    return result;
  } finally {
    await worker.close();
  }
}
