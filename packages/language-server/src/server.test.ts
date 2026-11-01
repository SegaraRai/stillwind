import { expect, test } from "vite-plus/test";

import { isGeneratedPath, uriKey } from "./server.ts";

const root = "file:///f/project";

test.each([
  ["file:///f/project/node_modules/pkg/index.js", true],
  ["file:///f/project/packages/app/node_modules/pkg/index.js", true],
  ["file:///f/project/target/debug/build.log", true],
  ["file:///f/project/.git/index", true],
  ["file:///f/project/app/.e2e/run-1/src/main.ts", true],
  ["file:///f/project/src/target.ts", false],
  ["file:///f/project/.env", false],
  ["file:///f/project/src/main.ts", false],
  ["file:///f/other/node_modules/pkg/index.js", false],
])("classifies %s", (uri, generated) => {
  expect(isGeneratedPath(uri, [root])).toBe(generated);
});

test("a workspace inside a dot directory stays visible", () => {
  expect(isGeneratedPath("file:///f/.work/project/src/main.ts", ["file:///f/.work/project/"])).toBe(
    false,
  );
});

test("URI keys treat editor and Node spellings of a file as equal", () => {
  expect(uriKey("file:///f%3A/app/(group)/a.ts")).toBe(uriKey("file:///F:/app/(group)/a.ts"));
  expect(uriKey("file:///f:/app/a.ts")).not.toBe(uriKey("file:///f:/app/b.ts"));
  expect(uriKey("file:///f:/app/100%.ts")).toBe("file:///f:/app/100%.ts");
});
