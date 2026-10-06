import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import type { BaseConfig } from "@stillwind/config";
import type { NativeDesignSystemLoader } from "@stillwind/native";

/**
 * Writes vocabulary declarations only when their generated content differs from the current file.
 *
 * @param loader - Initialized design-system loader whose vocabulary defines the declarations.
 * @param root - Absolute project directory for a relative declaration destination.
 * @param config - types selects the destination. False skips generation. Omission writes
 * src/stillwind.generated.d.ts. Missing parent directories are created.
 * @throws If generation, directory creation, or writing fails. A failed initial read is treated
 * as a reason to write the file.
 */
export function writeVocabulary(
  loader: NativeDesignSystemLoader,
  root: string,
  config: BaseConfig,
): void {
  if (config.types === false) {
    return;
  }
  const file = resolve(root, config.types ?? "src/stillwind.generated.d.ts");
  const content = loader.generateVocabularyTypes();
  try {
    if (readFileSync(file, "utf8") === content) {
      return;
    }
  } catch {}
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, content);
}
