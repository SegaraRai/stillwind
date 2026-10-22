import { readFile, readdir } from "node:fs/promises";

import { lintRules, resolveLintConfig } from "stillwind/config";
import { expect, test } from "vite-plus/test";

const content = new URL("../src/content/docs/docs/", import.meta.url);

test("documents every lint rule and compiler error at its public URL", async () => {
  const ledger = await readFile(
    new URL("../../../crates/diagnostics/src/compilation/code.rs", import.meta.url),
    "utf8",
  );
  const codes = [...ledger.matchAll(/=>\s*"(SW\d+)"/g)].map((match) => match[1]!);
  for (const [directory, names] of [
    ["lints", Object.keys(lintRules)],
    ["errors", codes],
  ] as const) {
    const files = await readdir(new URL(`${directory}/`, content));
    expect(files.filter((file) => file !== "index.mdx").toSorted()).toEqual(
      names.map((name) => `${name}.mdx`).toSorted(),
    );
    const index = await readFile(new URL(`${directory}/index.mdx`, content), "utf8");
    for (const name of names) {
      const page = await readFile(new URL(`${directory}/${name}.mdx`, content), "utf8");
      expect(page, name).toMatch(new RegExp(`^slug: docs/${directory}/${name}$`, "m"));
      expect(index, name).toContain(`](/docs/${directory}/${name}/)`);
      if (directory === "lints") {
        expect(page, name).toContain(`<LintRuleSummary name="${name}"`);
      } else {
        expect(page, name).toMatch(new RegExp(`^title: "${name}:`, "m"));
      }
    }
  }
});

test("lists the current recommended severity of every lint rule", async () => {
  const index = await readFile(new URL("lints/index.mdx", content), "utf8");
  const rows = [
    ...index.matchAll(/\[([^\]]+)\]\(\/docs\/lints\/[^)]+\)\s*\|[^\n]+?\|\s*(warn|deny|off)\s*\|/g),
  ];
  const documented = Object.fromEntries(rows.map(([, name, severity]) => [name, severity]));
  const actual = Object.fromEntries(
    Object.entries(resolveLintConfig(undefined).rules).map(([name, setting]) => [
      name,
      typeof setting === "string" ? setting : setting.severity,
    ]),
  );
  expect(documented).toEqual(actual);
});
