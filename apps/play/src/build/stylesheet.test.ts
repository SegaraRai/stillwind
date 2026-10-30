import { expect, test } from "vite-plus/test";

import { BuildFiles } from "./files.ts";
import { inlineImports } from "./stylesheet.ts";

test("workspace stylesheet imports are read on demand and inlined in place", async () => {
  const files = new Map([
    [
      "/workspace/app.css",
      '@import "./base.css";\n@import url("https://example.com/x.css");\n.app{}',
    ],
    ["/workspace/base.css", "@import './app.css';\n.base{}"],
  ]);
  const inventory = new BuildFiles(
    [...files.keys()].map((id) => [id, null]),
    async (id) => files.get(id)!,
  );
  expect(await inlineImports(inventory, "/workspace/app.css", new Set())).toBe(
    "@import './app.css';\n.base{}\n@import url(\"https://example.com/x.css\");\n.app{}",
  );
});
