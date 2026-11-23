import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { expect, test } from "vite-plus/test";

import cssData from "../css-data.json" with { type: "json" };

test("CSS custom data declares and describes every at-rule that the compiler offers", async () => {
  const directives = await readFile(
    resolve(import.meta.dirname, "../../../crates/compiler/src/query/stylesheet/directives.rs"),
    "utf8",
  );
  const descriptions = new Map(
    [...directives.matchAll(/^\s+name: "([a-z-]+)",$[\s\S]*?^\s+description: "(.*)",$/gm)].map(
      (match) => [`@${match[1]}`, JSON.parse(`"${match[2]}"`) as string],
    ),
  );
  expect(descriptions.size).toBeGreaterThan(0);

  const declared = new Map(
    cssData.atDirectives.map((directive) => [directive.name, directive.description.value]),
  );
  for (const [name, description] of descriptions) {
    expect(declared.get(name), name).toContain(description);
  }
  for (const [name, description] of declared) {
    expect(description, name).toMatch(/\S/);
  }
});
