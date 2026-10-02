import { registerBindingContract } from "./binding-contract.ts";
import { registerRefScopeContract } from "./ref-scope-contract.ts";

registerBindingContract("../dist/index.js");
registerRefScopeContract("../dist/index.js");
