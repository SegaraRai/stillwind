import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import type { CliResult } from "../src/result.ts";

const main = path.resolve(import.meta.dirname, "../dist/main.js");

test("fmt includes imported CSS, applies independent targets without lint, and preserves unchanged bytes", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-css-format-"));
  try {
    const config = path.join(root, "stillwind.config.json");
    const project = {
      css: "root.css",
      files: ["*.tsx"],
      typeFacts: false,
      lint: false,
      sort: { applyUtilities: true, applyStatements: true, variantBlocks: true },
    };
    await writeFile(config, JSON.stringify({ projects: [project] }));
    await writeFile(
      path.join(root, "root.css"),
      '@import "./local.css"; @utility a{display:block;} @utility b{margin:1px;} @utility c{padding:1px;} @custom-variant hover (&:hover); @custom-variant focus (&:focus);',
    );
    const file = path.join(root, "local.css");
    const source =
      "/* 日本語 🦀 */\r\n.a { @apply c; @apply b a; @variant focus { @apply c; } @variant hover { @apply a; } }\r\n";
    await writeFile(file, source);
    const check = await run(root, ["fmt", "--check", "--format", "json"]);
    expect(check.code, check.stderr).toBe(1);
    expect(JSON.parse(check.stdout).changed.map(path.normalize)).toEqual([file]);
    expect(await readFile(file, "utf8")).toBe(source);
    const result = await run(root, ["fmt"]);
    expect(result.code, result.stderr).toBe(0);
    const formatted = await readFile(file, "utf8");
    expect(formatted).toBe(
      source
        .replace("@apply c; @apply b a;", "@apply a b; @apply c;")
        .replace(
          "@variant focus { @apply c; } @variant hover { @apply a; }",
          "@variant hover { @apply a; } @variant focus { @apply c; }",
        ),
    );
    const before = (await stat(file)).mtimeMs;
    expect((await run(root, ["fmt", "--check"])).code).toBe(0);
    expect((await stat(file)).mtimeMs).toBe(before);
    await writeFile(config, JSON.stringify({ projects: [{ ...project, sort: false }] }));
    await writeFile(file, source);
    expect((await run(root, ["fmt", "--check"])).code).toBe(0);
    expect(await readFile(file, "utf8")).toBe(source);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

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
    const files = await run(root, ["fmt", "--check", "--format", "files"]);
    expect(files).toMatchObject({ code: 1, stdout: "entry.tsx\n", stderr: "" });
    const formatted = await run(root, ["fmt"]);
    expect(formatted.code, formatted.stderr).toBe(0);
    expect(formatted.stdout).toContain("entry.tsx: formatted");
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
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"prefer-canonical-utilities":"deny","prefer-sorted-utilities":"deny"}}}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@stillwind-theme-namespace --spacing-* [length] scale; @theme { --spacing: .25rem; } @utility block { display: block; } @utility flex { display: flex; } @utility p-* { padding: --value(--spacing-*, --spacing(), [length]); } @stillwind-slot utilities;",
    );
    const source =
      'import {style} from "stillwind/react";export const value=style("p-[16px] flex block");';
    await writeFile(file, source);
    expect((await run(root, ["lint"])).code).toBe(1);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(0);
    expect(await readFile(file, "utf8")).toContain('style("block flex p-4")');
    expect(
      JSON.parse(fixed.stdout).diagnostics.map(
        ({ report }: { report: { code: string } }) => report.code,
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
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],lint:{extends:[lintPresets.recommended],rules:{"unknown-utility":"deny","prefer-sorted-utilities":"deny"}}}]}',
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
      JSON.parse(remaining.stdout).diagnostics.map(
        ({ report }: { report: { code: string } }) => report.code,
      ),
    ).toEqual(["unknown-utility", "conflicting-utilities"]);
    expect(await readFile(file, "utf8")).toContain('style("block flex missing")');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("lint fixes remove redundant variants and add variable type hints", async () => {
  const root = await mkdtemp(
    path.resolve(import.meta.dirname, "../../host/.test-cli-candidate-fixes-"),
  );
  try {
    const file = path.join(root, "entry.tsx");
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"prefer-typed-variables":"deny"}}}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      "@custom-variant hover (&:hover); @custom-variant sm (@media (width >= 40rem)); @custom-variant md (@media (width >= 48rem)); @utility flex { display: flex; } @utility tone-* { color: --value([color]); } @utility tone-* { font-size: --value([length]); } @stillwind-slot utilities;",
    );
    await writeFile(
      file,
      'import {style} from "stillwind/react";export const value=style("hover:hover:flex md:sm:tone-(--x)");',
    );
    const report = await run(root, ["lint", "--format", "json"]);
    expect(
      JSON.parse(report.stdout).diagnostics.map(
        ({ report }: { report: { code: string } }) => report.code,
      ),
    ).toEqual(["redundant-variant", "redundant-variant", "prefer-typed-variables"]);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(0);
    expect(JSON.parse(fixed.stdout).diagnostics).toEqual([]);
    expect(await readFile(file, "utf8")).toContain('style("hover:flex md:tone-(color:--x)")');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("lint checks the CSS entry and imported user CSS, and lint fixes edit them", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-css-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"unknown-utility":"deny"}}}]}',
    );
    await writeFile(
      path.join(root, "root.css"),
      '@import "./local.css"; @import "pkg"; @custom-variant hover (&:hover); @utility flex { display: flex; } @stillwind-slot utilities;\n.btn { @apply missing; }\n',
    );
    const local = path.join(root, "local.css");
    await writeFile(local, ".card { @apply hover:hover:flex; }\n");
    const pkg = path.join(root, "node_modules", "pkg");
    await mkdir(pkg, { recursive: true });
    await writeFile(path.join(pkg, "package.json"), '{"name":"pkg","style":"index.css"}');
    await writeFile(path.join(pkg, "index.css"), ".pkg { @apply missing-too; }\n");
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";export const value=style("flex");',
    );
    /**
     * Lists the code and document of each finding in JSON output.
     * @param stdout - JSON printed by the CLI.
     * @returns Finding codes with the file names of their primary locations.
     */
    const findings = (stdout: string) =>
      JSON.parse(stdout).diagnostics.map(
        ({ report }: { report: { code: string; primary: { location: { uri: string } } } }) => [
          report.code,
          path.basename(fileURLToPath(report.primary.location.uri)),
        ],
      );
    const report = await run(root, ["lint", "--format", "json"]);
    expect(report.code, report.stderr).toBe(1);
    expect(findings(report.stdout)).toEqual([
      ["redundant-variant", "local.css"],
      ["unknown-utility", "root.css"],
    ]);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(1);
    expect(findings(fixed.stdout)).toEqual([["unknown-utility", "root.css"]]);
    expect(await readFile(local, "utf8")).toBe(".card { @apply hover:flex; }\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("lint reports stylesheet definitions and fixes implicit nesting selectors", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-css-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"prefer-explicit-nesting-selector":"deny"}}}]}',
    );
    const entry = path.join(root, "root.css");
    await writeFile(
      entry,
      '@import "./local.css"; @custom-variant hv (&:hover); @custom-variant kid (:hover, > .item); @utility flex { display: flex; } @stillwind-slot utilities;\n',
    );
    await writeFile(path.join(root, "local.css"), "@custom-variant hv (&:focus);\n");
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";export const value=style("flex");',
    );
    /**
     * Lists the code, primary document, and related documents of each finding in JSON
     * output.
     * @param stdout - JSON printed by the CLI.
     * @returns Finding codes with the file names of their locations.
     */
    const findings = (stdout: string) =>
      JSON.parse(stdout).diagnostics.map(
        ({
          report,
        }: {
          report: {
            code: string;
            primary: { location: { uri: string } };
            related: { location: { uri: string } }[];
          };
        }) => [
          report.code,
          path.basename(fileURLToPath(report.primary.location.uri)),
          report.related.map(({ location }) => path.basename(fileURLToPath(location.uri))),
        ],
      );
    const report = await run(root, ["lint", "--format", "json"]);
    expect(report.code, report.stderr).toBe(1);
    expect(findings(report.stdout)).toEqual([
      ["duplicate-definition", "local.css", ["root.css"]],
      ["prefer-explicit-nesting-selector", "root.css", []],
    ]);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(0);
    expect(findings(fixed.stdout)).toEqual([["duplicate-definition", "local.css", ["root.css"]]]);
    expect(await readFile(entry, "utf8")).toBe(
      '@import "./local.css"; @custom-variant hv (&:hover); @custom-variant kid (& :hover, & > .item); @utility flex { display: flex; } @stillwind-slot utilities;\n',
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("lint --fix replaces variant headers in stylesheets with @variant", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-css-"));
  try {
    await writeFile(
      path.join(root, "stillwind.config.ts"),
      'export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{rules:{"prefer-variant-directive":{severity:"warn",pxToRem:true}}}}]}',
    );
    const entry = path.join(root, "root.css");
    await writeFile(
      entry,
      `@import "stillwind";
.card {
  @media (prefers-color-scheme: dark) { color: red; }
  @media (hover: hover) {
    &:hover { color: blue; }
  }
  &:hover { color: green; }
  @media (min-width: 640px) { color: gray; }
}
`,
    );
    await writeFile(
      path.join(root, "entry.tsx"),
      'import {style} from "stillwind/react";export const value=style("");',
    );
    /**
     * Lists the code and variant of each finding in JSON output.
     * @param stdout - JSON printed by the CLI.
     * @returns Finding codes with the variant that each message names.
     */
    const findings = (stdout: string) =>
      JSON.parse(stdout).diagnostics.map(
        ({ report }: { report: { code: string; message: string } }) => [
          report.code,
          report.message.match(/variant `([^`]+)`/)?.[1],
        ],
      );
    const report = await run(root, ["lint", "--format", "json"]);
    expect(report.code, report.stderr).toBe(0);
    expect(findings(report.stdout)).toEqual([
      ["prefer-variant-directive", "dark"],
      ["prefer-variant-directive", "hover"],
      ["prefer-variant-directive", "sm"],
    ]);
    const fixed = await run(root, ["lint", "--fix", "--format", "json"]);
    expect(fixed.code, fixed.stderr).toBe(0);
    expect(findings(fixed.stdout)).toEqual([]);
    expect(await readFile(entry, "utf8")).toBe(
      `@import "stillwind";
.card {
  @variant dark { color: red; }
  @variant hover { color: blue; }
  &:hover { color: green; }
  @variant sm { color: gray; }
}
`,
    );
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
