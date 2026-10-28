import { expect, test } from "vite-plus/test";

import { normalizeWorkspacePath } from "./path.ts";

test.each([
  ["App.tsx", "App.tsx"],
  ["components\\Counter.tsx", "components/Counter.tsx"],
  ["/./components//./Counter.tsx", "components/Counter.tsx"],
  ["components/../lib/a.ts", "lib/a.ts"],
  ["a/b/../../c.ts", "c.ts"],
])("normalizes %s", (path, expected) => {
  expect(normalizeWorkspacePath(path)).toBe(expected);
});

test.each(["", "/", ".", "./", "a/..", "../App.tsx", "a/../../b.ts", "..\\b.ts"])(
  "rejects %j",
  (path) => {
    expect(normalizeWorkspacePath(path)).toBeNull();
  },
);
