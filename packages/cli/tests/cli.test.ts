import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import type { CliResult } from "../src/result.ts";
import { run } from "./run.ts";

test("CLI checks unused files, keeps overlapping project results, and isolates config logs from JSON", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'import {defineConfig} from "stillwind/config";export default defineConfig(async () => {console.log("configuration log");return {projects:[{css:"root.css",files:["*.tsx"],lint:{rules:{"unknown-utility":"deny"}}},{css:"root.css",files:["unused.tsx"],lint:{rules:{"unknown-utility":"deny"}}}]};});',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility red{color:red} @stillwind-slot utilities;",
    );
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";export const props=style("red");',
    );
    await writeFile(
      path.join(root, "unused.tsx"),
      'import {style} from "stillwind/react";export const props=style("missing");',
    );
    const result = await run(root, ["lint", "--format", "json"]);
    expect(result.code, result.stderr).toBe(1);
    expect(result.stderr).toContain("configuration log");
    const json = JSON.parse(result.stdout) as CliResult;
    // entry.tsx, unused.tsx, and the CSS entry root.css.
    expect(json.summary.files).toBe(3);
    expect(
      json.diagnostics.map(({ project, report }) => [
        project,
        path.basename(fileURLToPath(report.primary.location.uri)),
      ]),
    ).toEqual([
      [0, "unused.tsx"],
      [1, "unused.tsx"],
    ]);
    expect(json.diagnostics[0].report.code).toBe("unknown-utility");
    expect(json.diagnostics[0].docs).toMatch(/lints[/\\]unknown-utility(\.md)?$/);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(1);
    expect(JSON.parse(fixed.stdout).diagnostics).toEqual(json.diagnostics);
    expect((await run(root, ["lint", "entry.tsx"])).code).toBe(0);
    expect((await run(root, ["lint", "./entry.tsx"])).code).toBe(0);
    expect((await run(root, ["lint", path.join(root, "entry.tsx")])).code).toBe(0);
    expect((await run(root, ["lint", "absent.tsx"])).code).toBe(2);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("quiet preserves warning thresholds and CLI failures retain CSS reports", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-output-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.json"),
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["*.tsx"],
            typeFacts: false,
            lint: { rules: { "unknown-utility": "warn" } },
          },
        ],
      }),
    );
    await writeFile(path.join(root, "root.css"), "@stillwind-slot utilities;");
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";const value=style("missing");',
    );
    const quiet = await run(root, ["lint", "--quiet", "--max-warnings", "0", "--format", "json"]);
    expect(quiet.code, quiet.stderr).toBe(1);
    expect(JSON.parse(quiet.stdout)).toMatchObject({
      diagnostics: [],
      summary: {
        errors: 0,
        warnings: 1,
        warningLimitExceeded: true,
      },
    });
    const text = await run(root, ["lint", "--quiet", "--max-warnings=0"]);
    expect(text.stdout).toContain("exceeds --max-warnings");
    expect(text.stdout).not.toContain("Unknown utility");
    expect((await run(root, ["lint", "--quiet", "--max-warnings=1"])).code).toBe(0);
    await writeFile(path.join(root, "root.css"), "@theme {");
    for (const args of [["lint"], ["lint", "--fix"], ["fmt", "--check"]]) {
      const failed = await run(root, [...args, "--format", "json"]);
      expect(failed.code, failed.stderr).toBe(2);
      const result = JSON.parse(failed.stdout) as CliResult;
      expect(result).toMatchObject({
        command: args[0],
        exitCode: 2,
        diagnostics: [
          {
            report: {
              code: "SW13002",
              severity: "error",
              primary: {
                location: {
                  uri: expect.stringContaining("root.css"),
                },
              },
            },
          },
        ],
        failures: [
          {
            project: 0,
          },
        ],
      });
      expect(path.normalize(result.failures[0].file ?? "")).toBe(path.join(root, "entry.tsx"));
    }
    const cssText = await run(root, ["lint", "--color", "never"]);
    expect(cssText.stderr).toContain("root.css:1:1: error SW13002");
    expect(cssText.stderr).toContain("1 | @theme {");
    await writeFile(
      path.join(root, "healthy.css"),
      "@utility block {display:block;} @utility flex {display:flex;} @stillwind-slot utilities;",
    );
    const healthy = path.join(root, "healthy.tsx");
    await writeFile(
      healthy,
      'import {style} from "stillwind/react"; const card=style("flex block");',
    );
    await writeFile(
      path.join(root, "stillwind.config.json"),
      JSON.stringify({
        projects: [
          {
            css: "root.css",
            files: ["entry.tsx"],
            typeFacts: false,
          },
          {
            css: "healthy.css",
            files: ["healthy.tsx"],
            typeFacts: false,
          },
        ],
      }),
    );
    const partial = await run(root, ["fmt", "--format", "json"]);
    expect(partial.code).toBe(2);
    const outcomes: CliResult = JSON.parse(partial.stdout);
    expect(outcomes.changed.map(path.normalize)).toEqual([healthy]);
    expect(outcomes.failures).toHaveLength(1);
    expect(await readFile(healthy, "utf8")).toContain('style("block flex")');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("CLI reports unsupported targets and invalid project settings", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../integration/.test-cli-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.txt"]}]};',
    );
    await writeFile(path.join(root, "root.css"), "@stillwind-slot utilities;");
    await writeFile(path.join(root, "source.txt"), "plain text");
    const unsupported = await run(root, ["lint"]);
    expect(unsupported.code).toBe(2);
    expect(unsupported.stderr).toContain("Incomplete analysis");
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{files:["*.tsx"]}]};',
    );
    const missingCss = await run(root, ["lint"]);
    expect(missingCss.code).toBe(2);
    expect(missingCss.stderr).toContain("projects[0].css:");
  } finally {
    await rm(root, { recursive: true, force: true, maxRetries: 5 });
  }
});

test("fmt saves healthy targets around failures and check mode reports every outcome without writing", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-batch-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx","*.txt"],typeFacts:false}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility block { display: block; } @utility flex { display: flex; } @stillwind-slot utilities;",
    );
    const file = path.join(root, "a-entry.tsx");
    const source = 'import {style} from "stillwind/react";const card=style("flex block");';
    await writeFile(file, source);
    const brokenFile = path.join(root, "b-broken.tsx");
    const broken = source + "const broken=(";
    await writeFile(brokenFile, broken);
    const unsupported = path.join(root, "c-unsupported.txt");
    await writeFile(unsupported, "unsupported");
    const invalid = path.join(root, "d-invalid.tsx");
    await writeFile(invalid, Buffer.from([0xff]));
    const last = path.join(root, "z-entry.tsx");
    await writeFile(last, source);
    const unchanged = await stat(brokenFile);
    const check = await run(root, ["fmt", "--check", "--format", "json"]);
    expect(check.code).toBe(2);
    const planned: CliResult = JSON.parse(check.stdout);
    expect(planned.changed.map(path.normalize)).toEqual([file, last]);
    expect(planned.failures.map((failure) => path.normalize(failure.file!)).sort()).toEqual([
      brokenFile,
      unsupported,
      invalid,
    ]);
    expect(planned.summary).toMatchObject({
      files: 6,
      failures: 3,
      changed: 2,
    });
    expect(await readFile(file, "utf8")).toBe(source);
    expect(await readFile(last, "utf8")).toBe(source);
    const result = await run(root, ["fmt", "--format", "json"]);
    expect(result.code).toBe(2);
    expect(JSON.parse(result.stdout).changed.map(path.normalize)).toEqual([file, last]);
    expect(await readFile(file, "utf8")).toBe(source.replace("flex block", "block flex"));
    expect(await readFile(last, "utf8")).toBe(source.replace("flex block", "block flex"));
    expect(await readFile(brokenFile, "utf8")).toBe(broken);
    expect((await stat(brokenFile)).mtimeMs).toBe(unchanged.mtimeMs);
    expect(await readFile(unsupported, "utf8")).toBe("unsupported");
    expect(await readFile(invalid)).toEqual(Buffer.from([0xff]));
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx","*.txt"],typeFacts:false,lint:{rules:{"prefer-sorted-utilities":"deny"}}}]}',
    );
    await writeFile(file, source);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code).toBe(2);
    const linted: CliResult = JSON.parse(fixed.stdout);
    expect(linted.changed.map(path.normalize)).toEqual([file]);
    expect(linted.failures).toHaveLength(3);
    expect(await readFile(file, "utf8")).toBe(source.replace("flex block", "block flex"));
    expect(await readFile(brokenFile, "utf8")).toBe(broken);
    expect((await run(root, ["fmt", "absent.tsx"])).code).toBe(2);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("CLI help, version, invalid options and structured failures keep their exit contracts", async () => {
  const root = path.resolve(import.meta.dirname, "..");
  for (const args of [
    [],
    ["--help"],
    ["fmt", "--help"],
    ["lint", "--help"],
    ["lsp", "--help"],
    ["--version"],
  ]) {
    const result = await run(root, args);
    expect(result.code, result.stderr).toBe(0);
    expect(result.stdout).not.toBe("");
  }
  for (const args of [
    ["fmt", "--fix"],
    ["lint", "--check"],
    ["lint", "--format", "files"],
    ["fmt", "--quiet"],
    ["lint", "--max-warnings", "-1"],
    ["lint", "--max-warnings", "1.5"],
    ["lint", "--format", "xml"],
    ["lint", "--config"],
    ["lsp"],
    ["lsp", "--no-stdio"],
    ["missing"],
  ]) {
    const result = await run(root, args);
    expect(result.code, result.stdout).toBe(2);
    expect(result.stderr).not.toBe("");
  }
  for (const command of ["fmt", "lint"]) {
    const result = await run(root, [command, "--config", "missing-config.ts", "--format", "json"]);
    expect(result.code).toBe(2);
    expect(JSON.parse(result.stdout)).toMatchObject({
      command,
      exitCode: 2,
      failures: [
        {
          message: expect.any(String),
        },
      ],
      diagnostics: [],
      changed: [],
    });
  }
});

test("CLI selects CI annotations and explicit display modes without corrupting structured output", async () => {
  const root = path.resolve(import.meta.dirname, "..");
  const args = ["lint", "--config", "missing-config.ts"];
  const ci = await run(root, args, {
    GITHUB_ACTIONS: "true",
  });
  expect(ci.code).toBe(2);
  expect(ci.stdout).toMatch(/^::error/);
  const text = await run(root, [...args, "--format", "text", "--color", "never"], {
    GITHUB_ACTIONS: "true",
  });
  expect(text.stdout).toBe("");
  expect(text.stderr).not.toContain("\u001b");
  const colored = await run(root, [...args, "--format", "text", "--color", "always"], {
    NO_COLOR: "1",
  });
  expect(colored.stderr).toContain("\u001b[31m");
  const json = await run(root, ["--config", "missing-config.ts", "lint", "--format", "json"]);
  expect(JSON.parse(json.stdout).exitCode).toBe(2);
  const lsp = await run(root, ["lsp"], {
    GITHUB_ACTIONS: "true",
  });
  expect(lsp.stdout).toBe("");
  expect(lsp.code).toBe(2);
});

test("CLI rejects invalid display values before analysis or help and keeps JSON failures structured", async () => {
  const root = path.resolve(import.meta.dirname, "..");
  for (const args of [
    ["lint", "--format", "yaml"],
    ["fmt", "--format=xml"],
    ["lint", "--format"],
    ["lint", "--no-format"],
    ["lint", "--format", "yaml", "--help"],
    ["lint", "--color", "rainbow"],
    ["fmt", "--hyperlinks", "sometimes"],
  ]) {
    const result = await run(root, args);
    expect(result.code).toBe(2);
    expect(result.stderr).toContain("Invalid value for argument");
  }
  for (const args of [
    ["lint", "--config="],
    ["lint", "--no-config"],
    ["--fix", "fmt"],
    ["fmt", "--version"],
    ["__proto__"],
  ]) {
    expect((await run(root, args)).code).toBe(2);
  }
  for (const option of ["color", "hyperlinks"]) {
    const result = await run(root, ["lint", "--format", "json", `--${option}`, "invalid"]);
    expect(result.code).toBe(2);
    expect(result.stderr).toBe("");
    expect(JSON.parse(result.stdout)).toMatchObject({
      exitCode: 2,
      failures: [
        {
          message: expect.stringContaining(`--${option}`),
        },
      ],
    });
  }
});
