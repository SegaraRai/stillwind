import { expect, test } from "vite-plus/test";

import { completionSnippet } from "./completion.ts";

test("completion snippets escape source spelling before adding tab stops", () => {
  expect(completionSnippet("nth-[]:")).toBe("nth-[${1}]:${0}");
  expect(completionSnippet("[$}\\]:nth-[]:")).toBe("[\\$\\}\\\\]:nth-[${1}]:${0}");
  expect(completionSnippet("hover:")).toBeUndefined();
  expect(completionSnippet("[&[data-label='[]']]:hover:")).toBeUndefined();
});
