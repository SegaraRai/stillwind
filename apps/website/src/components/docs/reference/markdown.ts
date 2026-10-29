import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";

import type { GeneratedMarkdownComponentTransform } from "@cloudflare/nimbus-docs/types";
import { lintRules, type LintRule } from "stillwind/config";

import { compilerRevision, documentationCompiler } from "../../../lib/docs/reference/compiler";
import {
  compiledExampleMarkdown,
  compileExample,
  compileWith,
  type CompiledExampleOutput,
} from "./compiled-example";
import { compiledSource, compiledSourcePlugin } from "./compiled-source";
import {
  diagnosticExample,
  diagnosticMarkdown,
  type DiagnosticExample,
} from "./diagnostic-example";
import { lintRuleSummary, lintSummaryMarkdown } from "./lint-rule-summary";

export async function referenceMarkdown() {
  const [compiler, revision] = await Promise.all([documentationCompiler(), compilerRevision()]);
  const sources = await Promise.all(
    [
      "./markdown.ts",
      "./compiled-example.ts",
      "./compiled-source.ts",
      "./diagnostic-example.ts",
      "./lint-rule-summary.ts",
    ].map((path) => readFile(new URL(path, import.meta.url))),
  );
  const hash = createHash("sha256")
    .update(revision)
    .update(
      JSON.stringify(Object.keys(lintRules).map((name) => lintRuleSummary(name as LintRule))),
    );
  for (const source of sources) {
    hash.update(source);
  }
  const componentRevision = hash.digest("hex");
  const diagnostics = new Map<string, DiagnosticExample>();
  const failures: string[] = [];
  for (const kind of ["lints", "errors"]) {
    const directory = new URL(`../../../content/docs/docs/${kind}/`, import.meta.url);
    for (const file of await readdir(directory)) {
      if (!file.endsWith(".mdx")) {
        continue;
      }
      const page = await readFile(new URL(file, directory), "utf8");
      if (page.includes("<DiagnosticExample")) {
        const code = file.slice(0, -4);
        try {
          diagnostics.set(code, await diagnosticExample(code));
        } catch (error) {
          failures.push(String(error));
        }
      }
    }
  }
  // Markdown renderers run synchronously, so examples with a stylesheet, which
  // need their own compiler, are compiled before rendering.
  const stylesheetExamples = new Map<string, CompiledExampleOutput>();
  const pages = new URL("../../../content/docs/docs/", import.meta.url);
  for (const file of await readdir(pages, { recursive: true })) {
    if (!file.endsWith(".mdx")) {
      continue;
    }
    const page = await readFile(new URL(file.replaceAll("\\", "/"), pages), "utf8");
    for (const [, children] of page.matchAll(
      /<CompiledExample[^>]*>([\s\S]*?)<\/CompiledExample>/g,
    )) {
      const input = compiledSource(children!);
      if (input.stylesheet !== undefined) {
        try {
          stylesheetExamples.set(JSON.stringify(input), await compileExample(input));
        } catch (error) {
          failures.push(`${file}: ${String(error)}`);
        }
      }
    }
  }
  if (failures.length) {
    throw new Error(failures.join("\n"));
  }
  const componentMap = {
    // The lookup needs the compiler, so the Markdown version keeps only the page's prose.
    UtilityExplorer: {
      revision: componentRevision,
      render: () => "",
    },
    LintRuleSummary: {
      revision: componentRevision,
      render: ({ attrs }) => lintSummaryMarkdown(String(attrs.name) as LintRule, attrs.analysis),
    },
    CompiledExample: {
      revision: componentRevision,
      render: ({ attrs, children }) => {
        const input = compiledSource(children);
        const example =
          input.stylesheet === undefined
            ? compileWith(compiler, input.source!)
            : stylesheetExamples.get(JSON.stringify(input));
        if (!example) {
          throw new Error("Unknown CompiledExample stylesheet");
        }
        return compiledExampleMarkdown(example, attrs.hideCss === true || attrs.hideCss === "true");
      },
    },
    DiagnosticExample: {
      revision: componentRevision,
      render: ({ attrs }) => {
        const example = diagnostics.get(String(attrs.code));
        if (!example) {
          throw new Error(`Unknown diagnostic example: ${attrs.code}`);
        }
        return diagnosticMarkdown(example);
      },
    },
  } satisfies Record<string, GeneratedMarkdownComponentTransform>;
  return {
    componentMap,
    sourcePlugin: compiledSourcePlugin(diagnostics, componentRevision),
  };
}
