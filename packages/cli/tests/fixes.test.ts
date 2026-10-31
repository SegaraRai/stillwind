import { mkdir, mkdtemp, readFile, rm, stat, utimes, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vite-plus/test";

import { run } from "./run.ts";

test("fmt includes imported CSS, applies independent targets without lint, and preserves unchanged bytes", async () => {
  const root = await mkdtemp(path.resolve(import.meta.dirname, "../../host/.test-cli-css-format-"));
  try {
    const config = path.join(root, "stillwind.config.json");
    const project = {
      css: "root.css",
      files: ["*.tsx"],
      typeFacts: false,
      lint: false,
      sort: {
        applyUtilities: true,
        applyStatements: true,
        variantBlocks: true,
      },
    };
    await writeFile(
      config,
      JSON.stringify({
        projects: [project],
      }),
    );
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
    await writeFile(
      config,
      JSON.stringify({
        projects: [
          {
            ...project,
            sort: false,
          },
        ],
      }),
    );
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
      "@utility block{display:block} @utility p-4{padding:1rem} @stillwind-slot utilities;",
    );
    const file = path.join(root, "entry.tsx");
    const source =
      '\uFEFFimport {style} from "stillwind/react";\r\nexport const value=style("p-4 \\u0062lock");\r\n';
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
    expect(files).toMatchObject({
      code: 1,
      stdout: "entry.tsx\n",
      stderr: "",
    });
    const formatted = await run(root, ["fmt"]);
    expect(formatted.code, formatted.stderr).toBe(0);
    expect(formatted.stdout).toContain("entry.tsx: formatted");
    expect(await readFile(file, "utf8")).toBe(source.replace("p-4 \\u0062lock", "\\u0062lock p-4"));
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
    expect(await readFile(file, "utf8")).toContain('style("flex p-4")');
    expect(
      JSON.parse(fixed.stdout).diagnostics.map(
        ({
          report,
        }: {
          report: {
            code: string;
          };
        }) => report.code,
      ),
    ).toEqual([]);
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
        ({
          report,
        }: {
          report: {
            code: string;
          };
        }) => report.code,
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
      'import {lintPresets} from "stillwind/config";export default {projects:[{css:"root.css",files:["*.tsx"],typeFacts:false,lint:{extends:[lintPresets.recommended],rules:{"implicit-nesting-selector":"deny"}}}]}',
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
            primary: {
              location: {
                uri: string;
              };
            };
            related: {
              location: {
                uri: string;
              };
            }[];
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
      ["implicit-nesting-selector", "root.css", []],
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
        ({
          report,
        }: {
          report: {
            code: string;
            message: string;
          };
        }) => [report.code, report.message.match(/variant `([^`]+)`/)?.[1]],
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
