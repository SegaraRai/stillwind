import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import type { NativeDesignSystemLoader } from "@stillwind/native";
import type { StillwindOptions } from "stillwind/config";

export function writeVocabulary(
  loader: NativeDesignSystemLoader,
  root: string,
  options: StillwindOptions,
): void {
  if (options.types === false) {
    return;
  }
  const file = resolve(root, options.types ?? "src/stillwind.generated.d.ts");
  const content = loader.generateVocabularyTypes();
  try {
    if (readFileSync(file, "utf8") === content) {
      return;
    }
  } catch {}
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, content);
}
