import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";

import type { GeneratedMarkdownComponentTransform } from "@cloudflare/nimbus-docs/types";
import { lintRules, type LintRule } from "stillwind/config";

import { compilerRevision, documentationCompiler } from "../../../lib/docs/reference/compiler";
import { loadUtilityCatalog, utilityCatalogMarkdown } from "../../../lib/docs/utilities/catalog";
import { compiledExampleMarkdown, compileWith } from "./compiled-example";
import { compiledSource, compiledSourcePlugin } from "./compiled-source";
import {
  diagnosticExample,
  diagnosticMarkdown,
  type DiagnosticExample,
} from "./diagnostic-example";
import { lintRuleSummary, lintSummaryMarkdown } from "./lint-rule-summary";

export async function referenceMarkdown() {
  const [compiler, catalog, revision] = await Promise.all([
    documentationCompiler(),
    loadUtilityCatalog(),
    compilerRevision(),
  ]);
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
  if (failures.length) {
    throw new Error(failures.join("\n"));
  }
  const componentMap = {
    UtilityExplorer: {
      revision: catalog.revision + componentRevision,
      render: () => utilityCatalogMarkdown(catalog),
    },
    LintRuleSummary: {
      revision: componentRevision,
      render: ({ attrs }) => lintSummaryMarkdown(String(attrs.name) as LintRule, attrs.analysis),
    },
    CompiledExample: {
      revision: componentRevision,
      render: ({ children }) =>
        compiledExampleMarkdown(compileWith(compiler, compiledSource(children))),
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
