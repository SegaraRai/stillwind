import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

import type { LintResult } from "@stillwind/analysis/node";
import { expect, test, vi } from "vite-plus/test";

import { annotation, displayEnabled, renderResult } from "./output.ts";
import { cliResult, failureResult } from "./result.ts";

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
const analysis = {
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
const result = cliResult("lint", analysis);

const options = {
  format: "text" as const,
  color: "never" as const,
  hyperlinks: "never" as const,
  cwd: path.dirname(file),
};

test("text keeps source locations, related notes, and distinct project findings", () => {
  const text = renderResult(result, options);
  expect(text.split("Unknown utility")).toHaveLength(3);
  expect(text).toContain(`${path.basename(file)}:3:4: error unknown-utility`);
  expect(text).toContain("2 errors, 0 warnings in 1 file.");
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
  const text = renderResult(result, display);
  expect(text).toContain("\u001b[31merror");
  expect(text).toContain(`\u001b]8;;${pathToFileURL(file).href}#L3:4`);
  expect(
    JSON.parse(
      renderResult(result, {
        ...display,
        format: "json",
      }),
    ),
  ).toEqual(result);
  const github = renderResult(result, {
    ...display,
    format: "github",
  });
  expect(github).toContain("line=3,col=4,endLine=3,endColumn=8");
  expect(github).toContain("%2Cwith%25.tsx");
  expect(github).toContain("Defined here%0Anote: Use an existing utility");
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

test("GitHub paths use the workspace root and zero-length ranges remain point annotations", () => {
  vi.stubEnv("GITHUB_WORKSPACE", path.dirname(path.dirname(file)));
  try {
    const github = renderResult(result, {
      ...options,
      format: "github",
    });
    expect(github).toContain(
      `file=src/${path
        .basename(file)
        .replace(/,/g, "%2C")
        .replace(/%\.tsx$/, "%25.tsx")}`,
    );
    const point = cliResult("lint", {
      ...analysis,
      results: [
        {
          ...analysis.results[0],
          result: {
            ...analysis.results[0].result,
            reports: [
              {
                ...report,
                primary: {
                  ...report.primary,
                  location: {
                    ...location,
                    endColumn: location.startColumn,
                  },
                },
              },
            ],
          },
        },
      ],
    });
    expect(renderResult(point, { ...options, format: "github" })).toContain("line=3,col=4::");
    const document = cliResult("lint", {
      ...analysis,
      results: [
        {
          ...analysis.results[0],
          result: {
            ...analysis.results[0].result,
            reports: [
              {
                ...report,
                primary: {
                  ...report.primary,
                  scope: "document" as const,
                },
              },
            ],
          },
        },
      ],
    });
    expect(renderResult(document, options)).toContain(`${path.basename(file)}: error`);
    expect(renderResult(document, { ...options, format: "github" })).not.toContain(",line=");
  } finally {
    vi.unstubAllEnvs();
  }
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
  const formatted = cliResult(
    "fmt",
    {
      config: "config.ts",
      changed: [file],
      results: analysis.results,
    },
    { check: true },
  );
  expect(
    JSON.parse(
      renderResult(formatted, {
        ...options,
        format: "json",
        check: true,
      }),
    ),
  ).toEqual(formatted);
  expect(
    renderResult(formatted, {
      ...options,
      format: "github",
      check: true,
    }),
  ).toContain("Run stillwind fmt.");
  expect(renderResult(formatted, { ...options, format: "files" })).toBe(path.basename(file));
  expect(renderResult(formatted, { ...options, check: true })).toContain(
    "1 file needs formatting.",
  );
});

test("quiet counts warnings before hiding them and incomplete execution takes precedence", () => {
  const warningAnalysis = {
    ...analysis,
    results: [
      {
        ...analysis.results[0],
        result: {
          ...analysis.results[0].result,
          reports: [
            {
              ...report,
              severity: "warning",
            },
          ],
        },
      },
    ],
  };
  const quiet = cliResult("lint", warningAnalysis, {
    quiet: true,
    maxWarnings: 0,
  });
  expect(quiet).toMatchObject({
    diagnostics: [],
    summary: { errors: 0, warnings: 1, warningLimitExceeded: true },
    exitCode: 1,
  });
  expect(renderResult(quiet, options)).toContain("exceeds --max-warnings");
  expect(cliResult("lint", warningAnalysis, { maxWarnings: 1 }).exitCode).toBe(0);
  expect(cliResult("lint", { ...analysis, results: [] }).exitCode).toBe(2);
  const failed = failureResult(
    "fmt",
    Object.assign(new Error("Rejected source"), {
      results: analysis.results,
    }),
  );
  expect(failed).toMatchObject({
    command: "fmt",
    diagnostics: result.diagnostics,
    exitCode: 2,
  });
});
