import { expect, test } from "vite-plus/test";

import { registerBindingContract } from "./binding-contract.ts";
import { registerEditorContract } from "./editor-contract.ts";
import { registerCanonicalContract } from "./editor/canonical.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

const bindingTarget = new URL("../dist/index.js", import.meta.url).href;
registerBindingContract(bindingTarget);
registerCanonicalContract(bindingTarget);
registerEditorContract(bindingTarget);
registerRefScopeContract("../dist/index.js");

test("the public root loader prefers the native addon", async () => {
  const binding = await import("@stillwind/native");
  expect(binding.__napiBindingTarget).toBe("native");
});
