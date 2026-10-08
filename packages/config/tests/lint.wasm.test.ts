import { registerLintBindingContract } from "./lint-binding-contract.ts";

process.env.NAPI_RS_FORCE_WASI = "error";

registerLintBindingContract("wasm32-wasi");
