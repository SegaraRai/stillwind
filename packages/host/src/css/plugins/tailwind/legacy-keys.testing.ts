import { readFileSync } from "node:fs";

import { NativeDesignSystemLoader, type NativeLegacyThemeKey } from "@stillwind/native";

/**
 * Reads the v3 config keys of the built-in legacy metadata through a native design system, as
 * the plugin runtime receives them from the loaded stylesheet.
 *
 * @returns The namespace that each listed v3 config key reads.
 * @throws If the metadata cannot be read or loaded.
 */
export function builtinLegacyThemeKeys(): NativeLegacyThemeKey[] {
  const loader = new NativeDesignSystemLoader({});
  loader.provide(
    0,
    "tw-legacy-metadata.css",
    readFileSync(
      new URL("../../../../../core/css/builtin-metadata/tw-legacy-metadata.css", import.meta.url),
      "utf8",
    ),
  );
  return loader.editorQuery().legacyThemeKeys();
}
