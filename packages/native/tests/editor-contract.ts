import { registerCandidatesEditorContract } from "./editor/candidates.ts";
import { registerCanonicalEditorContract } from "./editor/canonical.ts";
import { registerPreviewsEditorContract } from "./editor/previews.ts";
import { registerStylesheetEditorContract } from "./editor/stylesheet.ts";

/**
 * Registers editor contracts for the selected native or WASM binding.
 *
 * @param specifier - Absolute URL of the binding module.
 */
export function registerEditorContract(specifier: string): void {
  registerCandidatesEditorContract(specifier);
  registerStylesheetEditorContract(specifier);
  registerPreviewsEditorContract(specifier);
  registerCanonicalEditorContract(specifier);
}
