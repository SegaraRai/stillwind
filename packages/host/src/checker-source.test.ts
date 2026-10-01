import { expect, test } from "vite-plus/test";

import { identityKey } from "./checker-source.ts";

test("checker declaration identity has its own namespace", () => {
  const key = identityKey({
    kind: "ref",
    moduleId: "src/a.tsx",
    start: 2,
    end: 6,
    bindingName: "item",
  });
  expect(Buffer.from(key.slice("stillwind-identity/v1:".length), "base64url").toString()).toBe(
    '{"kind":"ref","moduleId":"src/a.tsx","start":2,"end":6,"bindingName":"item"}',
  );
  expect(
    identityKey({ bindingName: "item", end: 6, start: 2, moduleId: "src/a.tsx", kind: "ref" }),
  ).toBe(key);
  expect(key).toMatch(/^stillwind-identity\/v1:/);
});
