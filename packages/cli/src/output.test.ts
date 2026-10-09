import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

import type { LintResult } from "@stillwind/analysis/node";
import { expect, test } from "vite-plus/test";

import { annotation, displayEnabled, renderFormat, renderLint } from "./output.ts";

const file = path.resolve(import.meta.dirname, "a file,with%.tsx");

const location = {
  uri: pathToFileURL(file).href,
  revision: "a",
  start: 0,
  end: 5,
  startUtf16: 0,
  endUtf16: 5,
  startLine: 2,
  startColumn: 3,
  endLine: 2,
  endColumn: 8,
};

const report = {
  code: "unknown-utility",
  severity: "error",
  message: "Unknown utility",
  primary: {
    scope: "range" as const,
    location,
  },
  related: [
    {
      scope: "range" as const,
      location,
      message: "Defined here",
    },
  ],
  notes: ["Use an existing utility"],
  actions: [],
};
const result = {
  config: "config.ts",
  exitCode: 1,
  results: [
    {
      project: 0,
      config: "config.ts",
      result: {
        entry: file,
        revision: 0,
        status: "complete",
        reports: [report, report],
      },
    },
    {
      project: 1,
      config: "config.ts",
      result: {
        entry: file,
        revision: 0,
        status: "complete",
        reports: [report],
      },
    },
  ],
} satisfies LintResult;

const options = {
  format: "text" as const,
  color: "never" as const,
  hyperlinks: "never" as const,
};

test("text keeps source locations, related notes, and distinct project findings", () => {
  const text = renderLint(result, options);
  expect(text.split("Unknown utility")).toHaveLength(3);
  expect(text).toContain(`${file}:3:4: error unknown-utility`);
  expect(text).toContain("Defined here");
  expect(text).toContain("Use an existing utility");
  expect(stripVTControlCharacters(text)).toBe(text);
});

test("explicit colors and source links do not alter JSON or GitHub formats", () => {
  const display = {
    ...options,
    color: "always" as const,
    hyperlinks: "always" as const,
  };
  const text = renderLint(result, display);
  expect(text).toContain("\u001b[31merror");
  expect(text).toContain(`\u001b]8;;${pathToFileURL(file).href}#L3:4`);
  expect(
    JSON.parse(
      renderLint(result, {
        ...display,
        format: "json",
      }),
    ),
  ).toEqual(result);
  const github = renderLint(result, {
    ...display,
    format: "github",
  });
  expect(github).toContain("line=3,col=4,endLine=3,endColumn=8");
  expect(github).toContain("%2Cwith%25.tsx");
  expect(github).toContain("Defined here%0AUse an existing utility");
  expect(stripVTControlCharacters(github)).toBe(github);
});

test("GitHub messages and metadata escape command delimiters", () => {
  expect(
    annotation(
      "error",
      {
        file: "a,b:c%",
        title: "title\n::warning",
      },
      "bad%\r\n::error",
    ),
  ).toBe("::error file=a%2Cb%3Ac%25,title=title%0A%3A%3Awarning::bad%25%0D%0A::error");
});

test("display auto mode distinguishes pipes, CI, and capable terminals", () => {
  expect(displayEnabled("auto", "color", false, {})).toBe(false);
  expect(displayEnabled("auto", "color", false, { GITHUB_ACTIONS: "true" })).toBe(true);
  expect(displayEnabled("auto", "color", true, { NO_COLOR: "" })).toBe(false);
  expect(displayEnabled("auto", "color", true, { FORCE_COLOR: "0" })).toBe(false);
  expect(displayEnabled("always", "color", false, { NO_COLOR: "1" })).toBe(true);
  expect(displayEnabled("auto", "hyperlinks", true, { WT_SESSION: "x" })).toBe(true);
  expect(displayEnabled("auto", "hyperlinks", true, { CI: "true", WT_SESSION: "x" })).toBe(false);
});

test("format check exposes changed files through JSON and CI annotations", () => {
  const formatted = {
    config: "config.ts",
    changed: [file],
    results: [],
    exitCode: 1 as const,
  };
  expect(
    JSON.parse(
      renderFormat(
        formatted,
        {
          ...options,
          format: "json",
        },
        true,
      ),
    ),
  ).toEqual(formatted);
  expect(
    renderFormat(
      formatted,
      {
        ...options,
        format: "github",
      },
      true,
    ),
  ).toContain("Run stillwind fmt.");
});
