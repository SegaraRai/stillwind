import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { expect, test } from "vite-plus/test";

import cssData from "../css-data.json" with { type: "json" };

test("CSS custom data declares every at-rule that the compiler offers", async () => {
  const directives = await readFile(
    resolve(import.meta.dirname, "../../../crates/compiler/src/query/stylesheet/directives.rs"),
    "utf8",
  );
  const names = [...directives.matchAll(/^\s+name: "([a-z-]+)",$/gm)].map((match) => match[1]);
  expect(names).not.toHaveLength(0);

  const declared = cssData.atDirectives.map((directive) => directive.name);
  expect(declared).toEqual(expect.arrayContaining(names.map((name) => `@${name}`)));
});
