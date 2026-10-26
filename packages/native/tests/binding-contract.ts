import { registerCompilationContract } from "./binding/compilation.ts";
import { registerDiagnosticsContract } from "./binding/diagnostics.ts";
import { registerPoliciesContract } from "./binding/policies.ts";
import { registerRestrictionsContract } from "./binding/restrictions.ts";
import { registerSessionsContract } from "./binding/sessions.ts";
import { registerSourceContract } from "./binding/source.ts";
import { registerTransportContract } from "./binding/transport.ts";

/**
 * Registers binding contracts for the selected native or WASM binding.
 *
 * @param bindingSpecifier - Absolute URL of the binding module.
 */
export function registerBindingContract(bindingSpecifier: string): void {
  registerCompilationContract(bindingSpecifier);
  registerDiagnosticsContract(bindingSpecifier);
  registerPoliciesContract(bindingSpecifier);
  registerRestrictionsContract(bindingSpecifier);
  registerSessionsContract(bindingSpecifier);
  registerSourceContract(bindingSpecifier);
  registerTransportContract(bindingSpecifier);
}
