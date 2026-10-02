import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

import { registerBindingContract } from "./binding-contract.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

const bindingTarget = pathToFileURL(
  createRequire(import.meta.url).resolve("../npm/wasm32-wasi"),
).href;
registerBindingContract(bindingTarget);
registerRefScopeContract(bindingTarget);
