import { mkdirSync, mkdtempSync } from "node:fs";
import { join, resolve } from "node:path";

/** Creates a unique run directory under the supplied parent, resolved from the repository root. */
export function createInventoryOutput(root: string, parent = "target/tailwind-inventory"): string {
  const directory = resolve(root, parent);
  mkdirSync(directory, { recursive: true });
  return mkdtempSync(join(directory, "run-"));
}
