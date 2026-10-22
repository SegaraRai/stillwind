import { readFile, readdir } from "node:fs/promises";

import { compilerOptions } from "@stillwind/host";
import { lintRules, type LintRule, type LintRules } from "stillwind/config";
import { expect, test } from "vite-plus/test";

import { compileWith } from "../src/components/docs/reference/compiled-example";
import { compiledSource } from "../src/components/docs/reference/compiled-source";
import {
  defaultStylesheet,
  documentationCompiler,
  reactSource,
  referenceCompiler,
} from "../src/lib/docs/reference/compiler";
import { projectReports } from "./support/project-reports";

const content = new URL("../src/content/docs/docs/", import.meta.url);
const allOff = Object.fromEntries(Object.keys(lintRules).map((name) => [name, "off"])) as Record<
  LintRule,
  "off"
>;
const lintPages = await Promise.all(
  (await readdir(new URL("lints/", content)))
    .filter((file) => file !== "index.mdx")
    .map(async (file) => ({
      file,
      text: await readFile(new URL(`lints/${file}`, content), "utf8"),
    })),
);

test.each(lintPages)(
  "$file reports its rule and accepts the corrected example",
  async ({ file, text }) => {
    const rule = /^(.+)\.mdx$/.exec(file)![1] as LintRule;
    expect(text).toContain(`<LintRuleSummary name="${rule}"`);
    const blocks = (section: string) => {
      const body = text.split(`## ${section}\n`)[1]?.split(/\n## /)[0] ?? "";
      return [...body.matchAll(/```(tsx?|css|json)([^\n]*)\n([\s\S]*?)```/g)];
    };
    const config = blocks("Configuration")[0]?.[3];
    const overrides = config
      ? (JSON.parse(config) as { lint: { rules: LintRules } }).lint.rules
      : {};
    const lint = { rules: { ...allOff, [rule]: "warn" as const, ...overrides } };
    const files = Object.fromEntries(
      blocks("Project files").map(([, , meta, source]) => [
        /title="([^"]+)"/.exec(meta)![1]!,
        source,
      ]),
    );
    for (const [section, reported] of [
      ["Reported", true],
      ["Fixed", false],
    ] as const) {
      const example = blocks(section)[0];
      expect(example, `${file}: ${section}`).toBeTruthy();
      const [, format, , body] = example!;
      const source = body.includes("import { style }") ? body : reactSource(body);
      const project = /<LintRuleSummary[^>]+analysis="project"/.test(text);
      const compiler = project
        ? undefined
        : await referenceCompiler(
            compilerOptions({ lint }),
            format === "css" ? `${defaultStylesheet}\n${body}` : defaultStylesheet,
          );
      const reports = project
        ? await projectReports(source, lint, files)
        : format === "css"
          ? compiler!.stylesheetReports()
          : compiler!.diagnose(source);
      const matches = reports.filter((report) => report.code === rule);
      expect(
        matches.length > 0,
        `${file}: ${section}: ${reports.map((report) => report.code).join(", ")}`,
      ).toBe(reported);
      if (reported) {
        expect(matches[0]!.primary.location).toBeTruthy();
      }
    }
  },
  30000,
);

test("the macro pages publish the actual compiled output of their own source examples", async () => {
  const compiler = await documentationCompiler();
  const files = (await readdir(new URL("macros/", content), { recursive: true }))
    .map((file) => file.replaceAll("\\", "/"))
    .filter((file) => file.endsWith(".mdx") && file !== "index.mdx");
  for (const file of files) {
    const page = await readFile(new URL(`macros/${file}`, content), "utf8");
    const route = file.replace(/(\/index)?\.mdx$/, "");
    const markdown = await readFile(
      new URL(`../dist/docs/macros/${route}/index.md`, import.meta.url),
      "utf8",
    );
    const examples = [...page.matchAll(/<CompiledExample>([\s\S]*?)<\/CompiledExample>/g)];
    expect(examples.length, file).toBeGreaterThan(0);
    for (const [, block] of examples) {
      const example = compileWith(compiler, compiledSource(block));
      expect(markdown, file).toContain(example.source.trim());
      expect(markdown, file).toContain(example.code.trim());
      if (example.css) {
        expect(markdown, file).toContain(example.css.trim());
      }
    }
  }
}, 30000);

test("source authoring errors reject the example and accept its correction", async () => {
  const compiler = await documentationCompiler();
  for (const code of ["SW10001", "SW10003", "SW10004", "SW10005", "SW11001"]) {
    const page = await readFile(new URL(`errors/${code}.mdx`, content), "utf8");
    const examples = [...page.matchAll(/```tsx?\n([\s\S]*?)```/g)].map((match) => match[1]!);
    expect(examples, code).toHaveLength(2);
    const source = (body: string) => (body.includes('from "stillwind/') ? body : reactSource(body));
    expect(
      compiler.compile(source(examples[0]!)).diagnostics.some((report) => report.code === code),
      code,
    ).toBe(true);
    expect(compiler.compile(source(examples[1]!)).diagnostics, code).toEqual([]);
  }
});

test("guide snippets compile against the stylesheet shown on their page", async () => {
  const guides = (await readdir(new URL("guides/", content))).map((file) => `guides/${file}`);
  for (const file of ["index.mdx", "why.md", "style.md", "how-styles-compile.md", ...guides]) {
    const page = await readFile(new URL(file, content), "utf8");
    const stylesheet = /```css\n([\s\S]*?)```/.exec(page)?.[1] ?? defaultStylesheet;
    const compiler = await referenceCompiler(
      compilerOptions({}),
      stylesheet.includes('@import "stillwind"')
        ? stylesheet
        : `${defaultStylesheet}\n${stylesheet}`,
    );
    for (const [, source] of page.matchAll(/```tsx?\n([\s\S]*?)```/g)) {
      if (!source.includes('from "stillwind/react"')) {
        continue;
      }
      expect(compiler.compile(source).diagnostics, file).toEqual([]);
      expect(
        compiler
          .diagnose(source)
          .filter(
            (report) =>
              !(report.code in lintRules) ||
              lintRules[report.code as LintRule].category === "input",
          ),
        file,
      ).toEqual([]);
    }
  }
}, 30000);
