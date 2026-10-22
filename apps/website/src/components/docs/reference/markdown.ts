import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import type { GeneratedMarkdownComponentTransform } from "@cloudflare/nimbus-docs/types";
import { lintRules, type LintRule } from "stillwind/config";

import { compilerRevision, documentationCompiler } from "../../../lib/docs/reference/compiler";
import { loadUtilityCatalog, utilityCatalogMarkdown } from "../../../lib/docs/utilities/catalog";
import { compiledExampleMarkdown, compileWith } from "./compiled-example";
import { compiledSource } from "./compiled-source";
import { compilerErrorMarkdown, type ErrorCategory } from "./compiler-error";
import { lintRuleSummary, lintSummaryMarkdown } from "./lint-rule-summary";

export async function referenceMarkdown(): Promise<
  Record<string, GeneratedMarkdownComponentTransform>
> {
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
      "./compiler-error.ts",
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
  return {
    UtilityExplorer: {
      revision: catalog.revision + componentRevision,
      render: () => utilityCatalogMarkdown(catalog),
    },
    LintRuleSummary: {
      revision: componentRevision,
      render: ({ attrs }) => lintSummaryMarkdown(String(attrs.name) as LintRule, attrs.analysis),
    },
    CompilerError: {
      revision: componentRevision,
      render: ({ attrs }) =>
        compilerErrorMarkdown(String(attrs.code), String(attrs.category) as ErrorCategory),
    },
    CompiledExample: {
      revision: componentRevision,
      render: ({ children }) =>
        compiledExampleMarkdown(compileWith(compiler, compiledSource(children))),
    },
  };
}
