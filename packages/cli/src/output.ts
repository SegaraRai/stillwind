import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { stripVTControlCharacters, styleText } from "node:util";

import type { CliDiagnostic, CliFailure, CliResult, ResultPolicy } from "./result.ts";
import type { SourceFrames } from "./source-frame.ts";

export type OutputFormat = "text" | "json" | "github" | "files";

export type DisplayMode = "auto" | "always" | "never";

export interface OutputOptions extends ResultPolicy {
  format: OutputFormat;
  color: DisplayMode;
  hyperlinks: DisplayMode;
  cwd?: string;
}

type Label = CliDiagnostic["report"]["primary"];

type Location = Label["location"];

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

/** Renders one CLI result independently of the original analysis graph. */
export function renderResult(
  result: CliResult,
  options: OutputOptions,
  stderr = false,
  frames?: SourceFrames,
): string {
  if (options.format === "json") {
    return JSON.stringify(result);
  }
  const root =
    options.format === "github"
      ? (process.env.GITHUB_WORKSPACE ?? options.cwd ?? process.cwd())
      : (options.cwd ?? process.cwd());
  const file = (uri: string) => {
    const displayed = displayPath(uri, root);
    return options.format === "github" ? displayed.replaceAll("\\", "/") : displayed;
  };
  if (options.format === "files") {
    return result.changed.map(file).join("\n");
  }
  const tty = !!(stderr ? process.stderr : process.stdout).isTTY;
  const color =
    options.format === "text" && displayEnabled(options.color, "color", tty, process.env);
  const hyperlinks =
    options.format === "text" && displayEnabled(options.hyperlinks, "hyperlinks", tty, process.env);
  const at = (label: Label) =>
    label.scope === "document"
      ? file(label.location.uri)
      : `${file(label.location.uri)}:${label.location.startLine + 1}:${label.location.startColumn + 1}`;
  const linked = (label: Label) => link(at(label), label.location, hyperlinks);
  const addFrame = (label: Label) => {
    const frame = frames?.render(label);
    if (frame) {
      lines.push(...frame.split("\n").map((line) => `  ${line}`));
    }
  };
  const lines: string[] = [];
  for (const { project, report } of result.diagnostics) {
    const message = clean(report.message);
    const details = [
      ...(report.primary.message ? [`note: ${clean(report.primary.message)}`] : []),
      ...report.related.map(
        (label) => `related: ${at(label)}${label.message ? `: ${clean(label.message)}` : ""}`,
      ),
      ...report.notes.map((note) => `note: ${clean(note)}`),
      ...report.actions.map((action) => `help: ${clean(action.title)}`),
    ];
    if (options.format === "github") {
      const location = report.primary.location;
      const properties: Record<string, string | number> = {
        file: file(location.uri),
        title: `project ${project + 1}: ${clean(report.code)}`,
      };
      if (report.primary.scope === "range") {
        properties.line = location.startLine + 1;
        properties.col = location.startColumn + 1;
        // GitHub annotation ends are inclusive; report ranges are exclusive.
        if (
          location.endColumn > 0 &&
          (location.endLine > location.startLine || location.endColumn > location.startColumn)
        ) {
          properties.endLine = location.endLine + 1;
          properties.endColumn = location.endColumn;
        } else if (location.endLine > location.startLine) {
          properties.endLine = location.endLine;
        }
      }
      lines.push(
        annotation(
          report.severity === "error" ? "error" : "warning",
          properties,
          [message, ...details].join("\n"),
        ),
      );
    } else {
      const severity = paint(
        report.severity,
        report.severity === "error" ? "red" : "yellow",
        color,
      );
      lines.push(
        `${linked(report.primary)}: ${severity} ${clean(report.code)} [project ${project + 1}]: ${message}`,
      );
      addFrame(report.primary);
      if (report.primary.message) {
        lines.push(`  note: ${clean(report.primary.message)}`);
      }
      for (const related of report.related) {
        lines.push(
          `  related: ${linked(related)}${related.message ? `: ${clean(related.message)}` : ""}`,
        );
        addFrame(related);
      }
      lines.push(...report.notes.map((note) => `  note: ${clean(note)}`));
      lines.push(...report.actions.map((action) => `  help: ${clean(action.title)}`));
      lines.push("");
    }
  }
  for (const failure of result.failures) {
    lines.push(renderFailure(failure, options.format === "github", file, color));
  }
  if (result.command === "fmt") {
    for (const changed of result.changed) {
      if (options.format === "github" && options.check) {
        lines.push(
          annotation(
            "error",
            { file: file(changed), title: "Stillwind formatting" },
            "Stillwind utilities need sorting. Run stillwind fmt.",
          ),
        );
      } else {
        const location = { uri: changed, startLine: 0, startColumn: 0 };
        lines.push(
          `${link(file(changed), location, hyperlinks)}${options.format === "text" ? (options.check ? ": needs formatting" : ": formatted") : ""}`,
        );
      }
    }
  }
  if (result.summary.warningLimitExceeded) {
    const message = `Warning count (${result.summary.warnings}) exceeds --max-warnings${options.maxWarnings === undefined ? "" : ` (${options.maxWarnings})`}.`;
    lines.push(
      options.format === "github"
        ? annotation("error", { title: "Stillwind warnings" }, message)
        : `error: ${message}`,
    );
  }
  if (options.format === "text") {
    if (lines.length && lines.at(-1) !== "") {
      lines.push("");
    }
    const { summary } = result;
    lines.push(
      result.command === "fmt" && !summary.failures
        ? options.check
          ? summary.changed
            ? `${count(summary.changed, "file")} ${summary.changed === 1 ? "needs" : "need"} formatting.`
            : `All selected files are formatted (${summary.files} checked).`
          : `Formatted ${count(summary.changed, "file")} (${summary.files} checked).`
        : `${count(summary.errors, "error")}, ${count(summary.warnings, "warning")} in ${count(summary.files, "file")}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`,
    );
    const suggestions = result.diagnostics.filter(({ report }) => report.actions.length > 0).length;
    if (result.command === "lint" && suggestions) {
      lines.push(
        `help: Suggested edits are available for ${count(suggestions, "diagnostic")}. Run stillwind lint --fix to apply automatic fixes.`,
      );
    }
  }
  return lines.join("\n");
}

/** Writes text execution failures to stderr and command results to stdout. */
export async function writeResult(result: CliResult, options: OutputOptions): Promise<void> {
  const stderr = options.format === "text" && result.failures.length > 0;
  const frames =
    options.format === "text" && result.diagnostics.length
      ? new (await import("./source-frame.ts")).SourceFrames()
      : undefined;
  const output = renderResult(result, options, stderr, frames);
  if (output) {
    (stderr ? process.stderr : process.stdout).write(output + "\n");
  }
}

function renderFailure(
  failure: CliFailure,
  github: boolean,
  file: (uri: string) => string,
  color: boolean,
): string {
  const details = failure.details.map((detail) => `${file(detail.file)}: ${clean(detail.message)}`);
  const project = failure.project === undefined ? "" : ` [project ${failure.project + 1}]`;
  return github
    ? annotation(
        "error",
        { ...(failure.file ? { file: file(failure.file) } : {}), title: `Stillwind${project}` },
        [clean(failure.message), ...details].join("\n"),
      )
    : `${failure.file ? `${file(failure.file)}: ` : ""}${paint("error", "red", color)}${project}: ${clean(failure.message)}${details.length ? `\n  ${details.join("\n  ")}` : ""}`;
}

function count(value: number, noun: string): string {
  return `${value} ${noun}${value === 1 ? "" : "s"}`;
}

function paint(text: string, color: "red" | "yellow", enabled: boolean): string {
  return enabled ? styleText(color, text, { validateStream: false }) : text;
}

function clean(text: string): string {
  return stripVTControlCharacters(text).replace(/\p{Cc}/gu, (character) =>
    character === "\n" || character === "\r" || character === "\t" ? character : "",
  );
}

function sourcePath(uri: string): string {
  try {
    return uri.startsWith("file:") ? fileURLToPath(uri) : uri;
  } catch {
    return uri;
  }
}

export function displayPath(uri: string, cwd: string): string {
  const file = sourcePath(uri);
  return clean(path.isAbsolute(file) ? path.relative(cwd, file) || "." : file);
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
