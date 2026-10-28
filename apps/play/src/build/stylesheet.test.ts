import { expect, test } from "vite-plus/test";

import { inlineImports } from "./stylesheet.ts";

test("workspace stylesheet imports are inlined in place", () => {
  const files = new Map([
    [
      "/workspace/app.css",
      '@import "./base.css";\n@import url("https://example.com/x.css");\n.app{}',
    ],
    ["/workspace/base.css", "@import './app.css';\n.base{}"],
  ]);
  expect(inlineImports(files, "/workspace/app.css", new Set())).toBe(
    "@import './app.css';\n.base{}\n@import url(\"https://example.com/x.css\");\n.app{}",
  );
});
