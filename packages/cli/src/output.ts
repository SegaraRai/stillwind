import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { stripVTControlCharacters, styleText } from "node:util";

import type { FormatResult, LintResult } from "@stillwind/analysis/node";
import { createConsola } from "consola";

export type OutputFormat = "text" | "json" | "github";
export type DisplayMode = "auto" | "always" | "never";
export interface OutputOptions {
  format: OutputFormat;
  color: DisplayMode;
  hyperlinks: DisplayMode;
}
type Report = LintResult["results"][number]["result"]["reports"][number];
type Location = Report["primary"]["location"];

export function displayEnabled(
  mode: DisplayMode,
  kind: "color" | "hyperlinks",
  tty: boolean,
  env: NodeJS.ProcessEnv,
): boolean {
  if (mode !== "auto") {
    return mode === "always";
  }
  if (kind === "color") {
    if (env.NO_COLOR !== undefined || env.FORCE_COLOR === "0") {
      return false;
    }
    if (env.FORCE_COLOR !== undefined) {
      return true;
    }
    return (
      env.TERM !== "dumb" && (tty || env.GITHUB_ACTIONS === "true" || env.GITLAB_CI === "true")
    );
  }
  return (
    tty &&
    !env.CI &&
    env.TERM !== "dumb" &&
    (env.TERM_PROGRAM === "vscode" ||
      env.TERM_PROGRAM === "iTerm.app" ||
      env.WT_SESSION !== undefined ||
      env.VTE_VERSION !== undefined ||
      env.TERM === "xterm-kitty")
  );
}

export function renderLint(result: LintResult, options: OutputOptions): string {
  if (options.format === "json") {
    return JSON.stringify(result);
  }
  const lines: string[] = [];
  const seen = new Set<string>();
  const color =
    options.format === "text" &&
    displayEnabled(options.color, "color", !!process.stdout.isTTY, process.env);
  const hyperlinks =
    options.format === "text" &&
    displayEnabled(options.hyperlinks, "hyperlinks", !!process.stdout.isTTY, process.env);
  for (const entry of result.results) {
    for (const report of entry.result.reports) {
      const key = JSON.stringify([
        entry.project,
        report.code,
        report.primary.location,
        report.message,
      ]);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      const message = clean(report.message);
      if (options.format === "github") {
        const location = report.primary.location;
        const properties: Record<string, string | number> = {
          file: sourcePath(location.uri),
          title: `project ${entry.project + 1}: ${clean(report.code)}`,
        };
        if (report.primary.scope === "range") {
          properties.line = location.startLine + 1;
          properties.col = location.startColumn + 1;
          // GitHub annotation ends are inclusive; host ranges are exclusive.
          if (location.endColumn > 0) {
            properties.endLine = location.endLine + 1;
            properties.endColumn = location.endColumn;
          }
        }
        const details = [
          message,
          ...report.related.map(
            (label) => `${at(label.location)}${label.message ? `: ${clean(label.message)}` : ""}`,
          ),
          ...report.notes.map(clean),
        ];
        lines.push(
          annotation(
            report.severity === "error" ? "error" : "warning",
            properties,
            details.join("\n"),
          ),
        );
      } else {
        const location = report.primary.location;
        const label = report.primary.scope === "document" ? sourcePath(location.uri) : at(location);
        const severity = color
          ? styleText(report.severity === "error" ? "red" : "yellow", report.severity, {
              validateStream: false,
            })
          : report.severity;
        lines.push(
          `[project ${entry.project + 1}] ${link(label, location, hyperlinks)}: ${severity} ${clean(report.code)}: ${message}`,
        );
        for (const related of report.related) {
          lines.push(
            `  ${link(at(related.location), related.location, hyperlinks)}${related.message ? `: ${clean(related.message)}` : ""}`,
          );
        }
        lines.push(...report.notes.map((note) => `  ${clean(note)}`));
      }
    }
    if (entry.result.status !== "complete") {
      const details = [`incomplete analysis${entry.result.error ? `: ${entry.result.error}` : ""}`];
      for (const dependency of entry.result.graph?.dependencies ?? []) {
        if (dependency.status === "unresolved" || dependency.status === "unsupported") {
          details.push(`${dependency.status}: ${dependency.specifier} from ${dependency.importer}`);
        }
      }
      for (const range of entry.result.graph?.unverified ?? []) {
        details.push(
          `unverified: ${range.moduleId}${range.start === undefined ? "" : `:${range.start}-${range.end}`}: ${range.reason}`,
        );
      }
      lines.push(
        options.format === "github"
          ? annotation(
              "error",
              { file: sourcePath(entry.result.entry), title: `project ${entry.project + 1}` },
              details.map(clean).join("\n"),
            )
          : `[project ${entry.project + 1}] ${sourcePath(entry.result.entry)}: ${details.map(clean).join("\n  ")}`,
      );
    }
  }
  if (!result.results.length) {
    const message = "No analysis targets matched the configured scopes";
    lines.push(options.format === "github" ? annotation("error", {}, message) : message);
  }
  return lines.join("\n");
}

export function renderFormat(result: FormatResult, options: OutputOptions, check: boolean): string {
  if (options.format === "json") {
    return JSON.stringify(result);
  }
  if (options.format === "github") {
    return check
      ? result.changed
          .map((file) =>
            annotation(
              "error",
              { file, title: "Stillwind formatting" },
              "Stillwind utilities need sorting. Run stillwind fmt.",
            ),
          )
          .join("\n")
      : result.changed.map(clean).join("\n");
  }
  const hyperlinks = displayEnabled(
    options.hyperlinks,
    "hyperlinks",
    !!process.stdout.isTTY,
    process.env,
  );
  return result.changed
    .map((file) => link(clean(file), { uri: file, startLine: 0, startColumn: 0 }, hyperlinks))
    .join("\n");
}

export function writeFailure(error: unknown, options: OutputOptions): void {
  const message = clean(error instanceof Error ? error.message : String(error));
  if (options.format === "json") {
    process.stdout.write(JSON.stringify({ exitCode: 2, error: { message } }) + "\n");
  } else if (options.format === "github") {
    process.stdout.write(annotation("error", { title: "Stillwind" }, message) + "\n");
  } else {
    createConsola({
      fancy: false,
      stdout: process.stderr,
      stderr: process.stderr,
      formatOptions: {
        colors: displayEnabled(options.color, "color", !!process.stderr.isTTY, process.env),
        date: false,
      },
    }).error(
      displayEnabled(options.color, "color", !!process.stderr.isTTY, process.env)
        ? styleText("red", message, { validateStream: false })
        : message,
    );
  }
}

function clean(text: string): string {
  return stripVTControlCharacters(text).replace(/\p{Cc}/gu, (character) =>
    character === "\n" || character === "\r" || character === "\t" ? character : "",
  );
}
function sourcePath(uri: string): string {
  return clean(uri.startsWith("file:") ? fileURLToPath(uri) : uri);
}
function at(location: Location): string {
  return `${sourcePath(location.uri)}:${location.startLine + 1}:${location.startColumn + 1}`;
}
function link(
  label: string,
  location: Pick<Location, "uri" | "startLine" | "startColumn">,
  enabled: boolean,
): string {
  const file = sourcePath(location.uri);
  if (!enabled || !path.isAbsolute(file)) {
    return label;
  }
  const url = pathToFileURL(file);
  url.hash = `L${location.startLine + 1}:${location.startColumn + 1}`;
  return `\u001b]8;;${url.href}\u001b\\${label}\u001b]8;;\u001b\\`;
}
export function annotation(
  level: "error" | "warning",
  properties: Record<string, string | number>,
  message: string,
): string {
  const escapeData = (value: string): string =>
    clean(value).replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
  const fields = Object.entries(properties)
    .map(
      ([key, value]) =>
        `${key}=${escapeData(String(value)).replace(/:/g, "%3A").replace(/,/g, "%2C")}`,
    )
    .join(",");
  return `::${level}${fields ? ` ${fields}` : ""}::${escapeData(message)}`;
}
