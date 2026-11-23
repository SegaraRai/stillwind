import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { stripVTControlCharacters, styleText } from "node:util";

import {
  isDesignCommand,
  type CliDiagnostic,
  type CliFailure,
  type CliResult,
  type ResultPolicy,
} from "./result.ts";
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
  // Project tags appear only when the reported items come from more than one project.
  const analysisProjects = new Set(
    [...result.diagnostics, ...result.previews, ...result.failures].flatMap(({ project }) =>
      project === undefined ? [] : [project],
    ),
  );
  const projectTag = (project: number | undefined) =>
    project !== undefined && analysisProjects.size > 1 ? ` [project ${project + 1}]` : "";
  for (const { project, report, docs } of result.diagnostics) {
    const message = clean(report.message);
    const details = [
      ...(report.primary.message ? [`note: ${clean(report.primary.message)}`] : []),
      ...report.related.map(
        (label) => `related: ${at(label)}${label.message ? `: ${clean(label.message)}` : ""}`,
      ),
      ...report.notes.map((note) => `note: ${clean(note)}`),
      ...report.actions.map(
        (action) => `help: ${clean(action.title)} (${action.autofix ? "autofix" : "suggestion"})`,
      ),
      ...(docs ? [`docs: ${docs}`] : []),
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
      const tag = projectTag(project);
      lines.push(
        `${paint(linked(report.primary), "bold", color)}: ${severity} ${paint(clean(report.code), "dim", color)}${tag ? paint(tag, "dim", color) : ""}: ${message}`,
      );
      addFrame(report.primary);
      if (report.primary.message) {
        lines.push(`  ${paint("note:", "cyan", color)} ${clean(report.primary.message)}`);
      }
      for (const related of report.related) {
        lines.push(
          `  ${paint("related:", "cyan", color)} ${linked(related)}${related.message ? `: ${clean(related.message)}` : ""}`,
        );
        addFrame(related);
      }
      lines.push(
        ...report.notes.map((note) => `  ${paint("note:", "cyan", color)} ${clean(note)}`),
      );
      lines.push(
        ...report.actions.map(
          (action) =>
            `  ${paint("help:", "cyan", color)} ${clean(action.title)} (${action.autofix ? "autofix" : "suggestion"})`,
        ),
      );
      if (docs) {
        lines.push(`  ${paint("docs:", "dim", color)} ${docs}`);
      }
      lines.push("");
    }
  }
  if (options.format === "text" && result.summary.omittedDiagnostics > 0) {
    lines.push(
      `... ${count(result.summary.omittedDiagnostics, "more diagnostic")} omitted${options.limit === undefined ? "" : ` (--limit ${options.limit})`}.`,
    );
    lines.push("");
  }
  for (const failure of result.failures) {
    lines.push(
      renderFailure(failure, projectTag(failure.project), options.format === "github", file, color),
    );
  }
  if (result.command === "fmt" || result.command === "check") {
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
        const status =
          options.format !== "text"
            ? ""
            : options.check
              ? ": needs formatting"
              : result.command === "check"
                ? ": updated"
                : ": formatted";
        lines.push(`${link(file(changed), location, hyperlinks)}${status}`);
      }
    }
  }
  if (result.command === "inspect") {
    for (const { project, location, css, valid, diagnostics, documentation } of result.previews) {
      const label = {
        scope: "range",
        location,
      } as Label;
      const tag = projectTag(project);
      lines.push(`${paint(linked(label), "bold", color)}${tag ? paint(tag, "dim", color) : ""}`);
      if (!valid && !diagnostics.length) {
        lines.push(`  ${paint("error", "red", color)}: CSS generation failed.`);
      }
      for (const { severity, code, message, docs } of diagnostics) {
        const renderedSeverity = paint(severity, severity === "error" ? "red" : "yellow", color);
        lines.push(`  ${renderedSeverity} ${paint(clean(code), "dim", color)}: ${clean(message)}`);
        if (docs) {
          lines.push(`  ${paint("docs:", "dim", color)} ${clean(docs)}`);
        }
      }
      const body = clean([css, documentation].filter(Boolean).join("\n")).trimEnd();
      if (body) {
        lines.push(...body.split("\n").map((line) => (line ? `  ${line}` : "")));
      }
      lines.push("");
    }
    if (!result.summary.previews && !result.summary.failures) {
      lines.push(
        result.emptyReason === "line-out-of-range"
          ? "The requested line is outside the selected file."
          : result.emptyReason === "column-out-of-range"
            ? "The requested column is outside the selected line."
            : result.emptyReason === "no-match-at-position"
              ? "No Stillwind style call overlaps the requested position (the file has styles at another line or column)."
              : "The selected source contains no Stillwind style calls or generated CSS.",
      );
    }
  }
  if (result.command === "info" && options.format === "text") {
    if (result.config) {
      lines.push(`config: ${file(result.config)}`);
      lines.push("");
    }
    for (const info of result.projects) {
      lines.push(`project ${info.project + 1}${info.name ? ` (${clean(info.name)})` : ""}`);
      lines.push(`  root: ${file(info.root)}`);
      lines.push(`  css: ${file(info.css)}`);
      lines.push(`  files: ${info.files.map(clean).join(", ")}`);
      if (info.exclude.length) {
        lines.push(`  exclude: ${info.exclude.map(clean).join(", ")}`);
      }
      lines.push(`  matched files: ${info.matchedFiles}`);
      if (info.plugins.length) {
        lines.push(`  plugins: ${info.plugins.map(clean).join(", ")}`);
      }
      lines.push(`  sort: ${info.sort ? "enabled" : "disabled"}`);
      lines.push(`  typeFacts: ${info.typeFacts ? "enabled" : "disabled"}`);
      const entries = Object.entries(info.lint);
      const denied = entries.filter(([, setting]) =>
        typeof setting === "string" ? setting === "deny" : setting.severity === "deny",
      ).length;
      const warned = entries.length - denied;
      lines.push(
        entries.length
          ? `  lint: ${count(entries.length, "active rule")} (${denied} deny, ${warned} warn)`
          : "  lint: disabled",
      );
      for (const [rule, setting] of entries) {
        const severity = typeof setting === "string" ? setting : setting.severity;
        lines.push(`    ${clean(rule)}: ${severity}`);
      }
      lines.push("");
    }
  }
  const projects = new Set([...result.tokens, ...result.candidates].map(({ project }) => project));
  const tag = (project: number) => (projects.size > 1 ? ` [project ${project + 1}]` : "");
  for (const { project, name, value, resolvedValue, source } of result.tokens) {
    lines.push(
      `${clean(name)}: ${clean(value)}${resolvedValue ? ` /* ${clean(resolvedValue)} */` : ""}${source === "default" ? " (default)" : ""}${tag(project)}`,
    );
  }
  for (const { project, name, kind, valid, css, suggestion, canonical } of result.candidates) {
    if (valid) {
      const label = kind === "marker" ? " (marker)" : kind === "pattern" ? " (pattern)" : "";
      lines.push(`${clean(name)}${label}${tag(project)}`);
      if (css) {
        lines.push(
          ...clean(css)
            .trimEnd()
            .split("\n")
            .map((line) => (line ? `  ${line}` : "")),
        );
      }
      if (canonical) {
        lines.push(`  help: The canonical spelling is ${clean(canonical)}.`);
      }
      if (css) {
        lines.push("");
      }
    } else {
      lines.push(`${clean(name)}${tag(project)}: ${paint(`unknown ${kind}`, "red", color)}`);
      if (suggestion) {
        lines.push(`  help: Did you mean ${clean(suggestion)}?`);
      }
      lines.push("");
    }
  }
  for (const target of result.unmatched) {
    lines.push(`${clean(target)}: ${paint("no match", "red", color)}`);
  }
  if (result.summary.warningLimitExceeded) {
    const message = `Warning count (${result.summary.warnings}) exceeds --max-warnings${options.maxWarnings === undefined ? "" : ` (${options.maxWarnings})`}.`;
    lines.push(
      options.format === "github"
        ? annotation("error", { title: "Stillwind warnings" }, message)
        : `${paint("error", "red", color)}: ${message}`,
    );
  }
  if (options.format === "text") {
    if (lines.length && lines.at(-1) !== "") {
      lines.push("");
    }
    const { summary } = result;
    lines.push(
      result.command === "info"
        ? `${count(summary.projects, "project")}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`
        : isDesignCommand(result.command)
          ? `${result.command === "theme" ? count(summary.tokens, "token") : count(summary.candidates, result.command === "variants" ? "variant" : "utility").replace("utilitys", "utilities")}${summary.invalidCandidates ? `; ${summary.invalidCandidates} unknown` : ""}${summary.unmatched ? `; ${count(summary.unmatched, "unmatched target")}` : ""}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`
          : result.command === "inspect"
            ? `${count(summary.previews, "preview")} in ${count(summary.files, "file")}${summary.invalidPreviews ? `; ${summary.invalidPreviews} failed to generate CSS` : ""}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`
            : result.command === "fmt" && !summary.failures
              ? options.check
                ? summary.changed
                  ? `${count(summary.changed, "file")} ${summary.changed === 1 ? "needs" : "need"} formatting.`
                  : `All selected files are formatted (${summary.files} checked).`
                : `Formatted ${count(summary.changed, "file")} (${summary.files} checked).`
              : result.command === "check"
                ? `${count(summary.errors, "error")}, ${count(summary.warnings, "warning")} in ${count(summary.files, "file")}${summary.changed ? (options.check ? `; ${count(summary.changed, "file")} ${summary.changed === 1 ? "needs" : "need"} formatting` : `; updated ${count(summary.changed, "file")}`) : ""}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`
                : `${count(summary.errors, "error")}, ${count(summary.warnings, "warning")} in ${count(summary.files, "file")}${summary.failures ? `; ${count(summary.failures, "execution failure")}` : ""}.`,
    );
    const autofixable = result.summary.autofixable;
    const manualSuggestions = result.summary.suggestions;
    if ((result.command === "lint" || result.command === "check") && autofixable) {
      lines.push(
        result.command === "check"
          ? `help: Automatic fixes are available for ${count(autofixable, "diagnostic")}. Run stillwind check --fix to apply automatic fixes and sort utilities.`
          : `help: Automatic fixes are available for ${count(autofixable, "diagnostic")}. Run stillwind lint --fix to apply automatic fixes.`,
      );
    } else if ((result.command === "lint" || result.command === "check") && manualSuggestions) {
      lines.push(
        `help: Manual suggestions are available for ${count(manualSuggestions, "diagnostic")}. Review them in the source; --fix does not apply them automatically.`,
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
  project: string,
  github: boolean,
  file: (uri: string) => string,
  color: boolean,
): string {
  const details = failure.details.map((detail) => `${file(detail.file)}: ${clean(detail.message)}`);
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

function paint(
  text: string,
  color: "red" | "yellow" | "cyan" | "dim" | "bold",
  enabled: boolean,
): string {
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
