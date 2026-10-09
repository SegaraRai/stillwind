import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "vite-plus/test";

const main = path.resolve(import.meta.dirname, "../dist/main.js");

test("fmt checks and sorts with byte preservation; unchanged fmt and lint fixes preserve mtime", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-format-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility block{display:block} @utility flex{display:flex} @stillwind-slot utilities;",
    );
    const file = path.join(root, "entry.tsx");
    const source =
      '\uFEFFimport {style} from "stillwind/react";\r\nexport const value=style("flex \\u0062lock");\r\n';
    await writeFile(file, source);
    const timestamp = new Date("2020-01-01T00:00:00Z");
    await utimes(file, timestamp, timestamp);
    const before = (await stat(file)).mtimeMs;
    const check = await run(root, ["fmt", "--check", "--format", "json"]);
    expect(
      JSON.parse(check.stdout).changed.map((changed: string) => path.normalize(changed)),
    ).toEqual([file]);
    expect(check.code, check.stderr).toBe(1);
    expect(await readFile(file, "utf8")).toBe(source);
    expect((await stat(file)).mtimeMs).toBe(before);
    const formatted = await run(root, ["fmt"]);
    expect(formatted.code, formatted.stderr).toBe(0);
    expect(await readFile(file, "utf8")).toBe(
      source.replace("flex \\u0062lock", "\\u0062lock flex"),
    );
    await utimes(file, timestamp, timestamp);
    for (const args of [["fmt"], ["fmt", "--check"], ["lint"], ["lint", "--fix"]]) {
      const result = await run(root, args);
      expect(result.code, result.stderr).toBe(0);
      expect((await stat(file)).mtimeMs).toBe(before);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("lint fixes report final findings and conflicting projects cannot write", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-fixes-"));
  try {
    const config = path.join(root, "stillwind.config.ts");
    const file = path.join(root, "entry.tsx");
    await writeFile(
      config,
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"canonical-utilities":"deny","sort-utilities":"deny"}}}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@theme { --spacing: .25rem; } @utility block { display: block; } @utility flex { display: flex; } @utility p-* { padding: --value(--spacing-*, --spacing(), [length]); } @stillwind-slot utilities;",
    );
    const source =
      'import {style} from "stillwind/react";export const value=style("p-[16px] flex block");';
    await writeFile(file, source);
    expect((await run(root, ["lint"])).code).toBe(1);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(0);
    expect(await readFile(file, "utf8")).toContain('style("block flex p-4")');
    expect(
      JSON.parse(fixed.stdout).results[0].result.reports.map(
        (report: { code: string }) => report.code,
      ),
    ).toEqual(["conflicting-utilities"]);
    await writeFile(
      config,
      'export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false},{css:"root.css",files:["*.tsx"],typeFacts:false,sort:{stringLiterals:false}}]}',
    );
    await writeFile(file, source);
    const conflict = await run(root, ["fmt"]);
    expect(conflict.code).toBe(2);
    expect(conflict.stderr).toContain("Conflicting project rewrites");
    expect(await readFile(file, "utf8")).toBe(source);
    await writeFile(
      config,
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],lint:{extends:[lintPresets.recommended],rules:{"unknown-utility":"deny","sort-utilities":"deny"}}}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@utility block { display: block; } @utility flex { display: flex; } @stillwind-slot utilities;",
    );
    await writeFile(
      file,
      'import {style} from "stillwind/react";export const value=style("flex block missing");',
    );
    const remaining = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(remaining.code, remaining.stderr).toBe(1);
    expect(
      JSON.parse(remaining.stdout).results[0].result.reports.map(
        (report: { code: string }) => report.code,
      ),
    ).toEqual(["unknown-utility", "conflicting-utilities"]);
    expect(await readFile(file, "utf8")).toContain('style("block flex missing")');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

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
    const json = JSON.parse(result.stdout) as {
      results: {
        project: number;
        result: {
          entry: string;
          reports: {
            code: string;
          }[];
        };
      }[];
    };
    expect(json.results.map((entry) => [entry.project, path.basename(entry.result.entry)])).toEqual(
      [
        [0, "entry.tsx"],
        [0, "unused.tsx"],
        [1, "unused.tsx"],
      ],
    );
    expect(json.results[1].result.reports[0].code).toBe("unknown-utility");
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(1);
    expect(
      JSON.parse(fixed.stdout).results.map(
        (entry: { project: number; result: { entry: string } }) => [
          entry.project,
          path.basename(entry.result.entry),
        ],
      ),
    ).toEqual(json.results.map((entry) => [entry.project, path.basename(entry.result.entry)]));
    expect((await run(root, ["lint", "entry.tsx"])).code).toBe(0);
    expect((await run(root, ["lint", "absent.tsx"])).code).toBe(2);
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
    expect(unsupported.stdout).toContain("incomplete analysis");
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

test("failed batch planning preserves previously planned sources and reports unresolved targets", async () => {
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
    await writeFile(path.join(root, "z-unsupported.txt"), "unsupported");
    const result = await run(root, ["fmt"]);
    expect(result.code).toBe(2);
    expect(result.stderr).toContain("Incomplete analysis");
    expect(await readFile(file, "utf8")).toBe(source);
    expect((await run(root, ["fmt", "absent.tsx"])).code).toBe(2);
    await rm(path.join(root, "z-unsupported.txt"));
    const broken = source + "const broken=(";
    await writeFile(file, broken);
    const syntaxError = await run(root, ["fmt"]);
    expect(syntaxError.code).toBe(2);
    expect(syntaxError.stderr).toContain("JavaScript parse error");
    expect(await readFile(file, "utf8")).toBe(broken);
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
    expect(JSON.parse(result.stdout)).toEqual({
      exitCode: 2,
      error: {
        message: expect.any(String),
      },
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
    expect(JSON.parse(result.stdout)).toEqual({
      exitCode: 2,
      error: {
        message: expect.stringContaining(`--${option}`),
      },
    });
  }
});

async function run(
  cwd: string,
  args: string[],
  env: NodeJS.ProcessEnv = {},
): Promise<{
  code: number | null;
  stdout: string;
  stderr: string;
}> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [main, ...args], {
      cwd,
      windowsHide: true,
      env: {
        ...process.env,
        ...env,
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });
    child.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        code,
        stdout,
        stderr,
      });
    });
  });
}
