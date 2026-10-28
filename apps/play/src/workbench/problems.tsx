import { style } from "stillwind/react";
import type * as vscode from "vscode";

import { fileIcon, getFileInfo } from "../file-info.ts";

/**
 * Diagnostic reported for one workspace file.
 */
export type Problem = {
  uri: vscode.Uri;
  diagnostic: vscode.Diagnostic;
};

/** Matches `vscode.DiagnosticSeverity`, whose enum the playground cannot import at runtime. */
const ERROR = 0;
const WARNING = 1;

/**
 * Counts problems by severity.
 *
 * @returns Error and warning counts. Information and hints are counted in neither.
 */
export function countProblems(problems: Problem[]): { errors: number; warnings: number } {
  let errors = 0;
  let warnings = 0;
  for (const { diagnostic } of problems) {
    if (diagnostic.severity === ERROR) {
      errors++;
    } else if (diagnostic.severity === WARNING) {
      warnings++;
    }
  }
  return {
    errors,
    warnings,
  };
}

/**
 * Shows the severity icon of a diagnostic.
 *
 * @param props.severity - A `vscode.DiagnosticSeverity` value.
 */
export function SeverityIcon({ severity }: { severity: vscode.DiagnosticSeverity }) {
  const base = style("icon-control shrink-0");
  if (severity === ERROR) {
    return <span aria-label="Error" {...style(base, "icon-lucide--circle-x text-danger")} />;
  }
  if (severity === WARNING) {
    return (
      <span aria-label="Warning" {...style(base, "icon-lucide--triangle-alert text-warning")} />
    );
  }
  return <span aria-label="Information" {...style(base, "icon-lucide--info text-accent")} />;
}

/**
 * Lists diagnostics grouped by file, like the Problems panel of VS Code.
 *
 * @param props.problems - Diagnostics in the order that the language service reports them.
 * @param props.onSelect - Called with the file and the zero-based range of a clicked diagnostic.
 */
export function ProblemsPanel({
  problems,
  onSelect,
}: {
  problems: Problem[];
  onSelect: (file: string, range: vscode.Range) => void;
}) {
  const groups = new Map<string, Problem[]>();
  for (const problem of problems) {
    groups.set(problem.uri.path, [...(groups.get(problem.uri.path) ?? []), problem]);
  }
  return (
    <section
      aria-label="Problems"
      {...style("flex min-block-0 min-inline-0 flex-col bg-background border-divider border-bs")}
    >
      <div {...style("flex px-4 block-9 shrink-0 items-center gap-2 select-none")}>
        <h2 {...style("text-foreground pane-title")}>Problems</h2>
        <span
          {...style(
            "px-1.5 min-inline-4.5 text-center text-foreground text-label font-semibold bg-surface-muted rounded-full",
          )}
        >
          {problems.length}
        </span>
      </div>
      <div {...style("overflow-y-auto pbe-2 min-block-0 flex-1", { class: "diagnostics-output" })}>
        {problems.length === 0 ? (
          <p {...style("flex px-4 py-1 items-center gap-2 text-muted")}>
            <span
              aria-hidden="true"
              {...style("icon-control icon-lucide--circle-check text-success")}
            />
            No problems have been detected in the workspace.
          </p>
        ) : (
          <ul>
            {[...groups].map(([file, items]) => (
              <li key={file}>
                <div {...style("flex px-4 block-5.5 items-center gap-1.5 text-ui")}>
                  <span
                    aria-hidden="true"
                    {...style("icon-control icon-lucide--chevron-down text-muted")}
                  />
                  <span
                    data-type={getFileInfo(file).type}
                    {...style(fileIcon, "icon-control shrink-0")}
                  />
                  <span {...style("text-foreground")}>{file.split("/").at(-1)}</span>
                  <span {...style("truncate text-caption text-subtle")}>
                    {file.replace("/workspace/", "").split("/").slice(0, -1).join("/")}
                  </span>
                  <span
                    {...style(
                      "px-1.5 min-inline-4.5 text-center text-foreground text-label font-semibold bg-surface-muted rounded-full",
                    )}
                  >
                    {items.length}
                  </span>
                </div>
                <ul>
                  {items.map(({ diagnostic }, index) => {
                    const code =
                      typeof diagnostic.code === "object" ? diagnostic.code.value : diagnostic.code;
                    return (
                      <li key={index}>
                        <button
                          type="button"
                          onClick={() => onSelect(file, diagnostic.range)}
                          {...style(
                            "flex py-0.5 ps-11 pe-4 inline-full min-block-5.5 items-baseline gap-1.5 text-start text-ui cursor-pointer",
                            { "hover:": "bg-surface-muted/60" },
                          )}
                        >
                          <span {...style("self-center")}>
                            <SeverityIcon severity={diagnostic.severity} />
                          </span>
                          <span {...style("min-inline-0 text-foreground")}>
                            {diagnostic.message}
                          </span>
                          <span {...style("shrink-0 text-caption text-subtle whitespace-nowrap")}>
                            {diagnostic.source}
                            {code !== undefined && `(${code})`} [Ln{" "}
                            {diagnostic.range.start.line + 1}, Col{" "}
                            {diagnostic.range.start.character + 1}]
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
