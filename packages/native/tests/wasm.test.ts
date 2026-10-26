import { expect, test } from "vite-plus/test";

import { registerBindingContract } from "./binding-contract.ts";
import { registerEditorContract } from "./editor-contract.ts";
import { registerCanonicalContract } from "./editor/canonical.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

process.env.NAPI_RS_FORCE_WASI = "error";

const bindingTarget = new URL("../dist/index.js", import.meta.url).href;
registerBindingContract(bindingTarget);
registerCanonicalContract(bindingTarget);
registerEditorContract(bindingTarget);
registerRefScopeContract(bindingTarget);

test("the public root loader selects WASI when it is required", async () => {
  const binding = await import(bindingTarget);
  expect(binding.__napiBindingTarget).toBe("wasm32-wasi");
});
