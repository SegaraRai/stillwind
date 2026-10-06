import { expect, test } from "vite-plus/test";

import { registerBindingContract } from "./binding-contract.ts";
import { registerEditorContract } from "./editor-contract.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

registerBindingContract("../dist/index.js");
registerEditorContract("../dist/index.js");
registerRefScopeContract("../dist/index.js");

test("the public root loader prefers the native addon", async () => {
  const binding = await import("@stillwind/native");
  expect(binding.__napiBindingTarget).toBe("native");
});
