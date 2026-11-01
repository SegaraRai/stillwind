import { expect, test } from "vite-plus/test";

import { uriKey } from "./server.ts";

test("URI keys treat editor and Node spellings of a file as equal", () => {
  expect(uriKey("file:///f%3A/app/(group)/a.ts")).toBe(uriKey("file:///F:/app/(group)/a.ts"));
  expect(uriKey("file:///f:/app/a.ts")).not.toBe(uriKey("file:///f:/app/b.ts"));
  expect(uriKey("file:///f:/app/100%.ts")).toBe("file:///f:/app/100%.ts");
});
