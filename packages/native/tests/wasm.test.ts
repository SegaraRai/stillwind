import { expect, test } from "vite-plus/test";

import { registerBindingContract } from "./binding-contract.ts";
import { registerEditorContract } from "./editor-contract.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

process.env.NAPI_RS_FORCE_WASI = "error";

const bindingTarget = "../dist/index.js";
registerBindingContract(bindingTarget);
registerEditorContract(bindingTarget);
registerRefScopeContract(bindingTarget);

test("the public root loader selects WASI when it is required", async () => {
  const binding = await import(bindingTarget);
  expect(binding.__napiBindingTarget).toBe("wasm32-wasi");
});
