import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { codeFrameColumns } from "@babel/code-frame";
import { fingerprint } from "@stillwind/native";
import stringWidth from "string-width";

import type { CliDiagnostic } from "./result.ts";

type Label = CliDiagnostic["report"]["primary"];

/** Reads each document once and only displays the revision named by a report. */
export class SourceFrames {
  readonly #sources = new Map<string, { text: string; revision: string } | null>();

  render(label: Label): string | undefined {
    if (label.scope !== "range") {
      return undefined;
    }
    const { uri, revision } = label.location;
    if (!this.#sources.has(uri)) {
      try {
        const file = uri.startsWith("file:") ? fileURLToPath(uri) : uri;
        if (!path.isAbsolute(file)) {
          return undefined;
        }
        const text = readFileSync(file, "utf8");
        this.#sources.set(uri, { text, revision: fingerprint(text) });
      } catch {
        this.#sources.set(uri, null);
      }
    }
    const source = this.#sources.get(uri);
    return source?.revision === revision ? sourceFrame(source.text, label) : undefined;
  }
}

/** Adapts UTF-16 ranges to terminal widths while Babel supplies the frame and gutters. */
export function sourceFrame(source: string, label: Label): string | undefined {
  if (label.scope !== "range") {
    return undefined;
  }
  const location = label.location;
  const lines = source.split("\n").map((line) => line.replace(/\r$/, ""));
  const { startLine, startColumn } = location;
  let { endLine, endColumn } = location;
  if (
    startLine < 0 ||
    endLine < startLine ||
    endLine >= lines.length ||
    startColumn < 0 ||
    startColumn > lines[startLine].length ||
    endColumn < 0 ||
    endColumn > lines[endLine].length ||
    (startLine === endLine && endColumn < startColumn)
  ) {
    return undefined;
  }
  if (endLine > startLine && endColumn === 0) {
    endColumn = lines[--endLine].length;
  }
  const truncated = endLine > startLine + 3;
  if (truncated) {
    endLine = startLine + 3;
    endColumn = lines[endLine].length;
  }
  const display = (text: string) =>
    text.replace(/\t/g, "    ").replace(/[\p{Cc}\u2028\u2029]/gu, " ");
  const frame = codeFrameColumns(
    lines.map(display).join("\n"),
    {
      start: { line: startLine + 1, column: 1 },
      end: { line: endLine + 1, column: lines[endLine].length + 1 },
    },
    { linesAbove: 1, linesBelow: truncated ? 0 : 1, highlightCode: false },
  );
  let current = startLine;
  const rendered = frame
    .split("\n")
    .map((line) => {
      const numbered = /^>\s+(\d+) \|/.exec(line);
      if (numbered) {
        current = Number(numbered[1]) - 1;
      }
      const marker = /^(\s+\| ) *\^+$/.exec(line);
      if (!marker) {
        return line;
      }
      const start = current === startLine ? startColumn : 0;
      const end = current === endLine ? endColumn : lines[current].length;
      const before = stringWidth(display(lines[current].slice(0, start)));
      const width = Math.max(1, stringWidth(display(lines[current].slice(start, end))));
      return `${marker[1]}${" ".repeat(before)}${"^".repeat(width)}`;
    })
    .join("\n");
  return truncated ? `${rendered}\n  …` : rendered;
}
