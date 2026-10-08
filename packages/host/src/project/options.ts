import type { BaseConfig } from "@stillwind/config";
import type { NativeCompilerOptions } from "@stillwind/native";

/**
 * Selects the design-system stylesheet request.
 *
 * @param config - Compiler settings. css takes precedence over the theme alias.
 * @returns The authored path without resolving it. The caller applies the project root.
 * @throws If neither setting supplies a nonempty stylesheet path.
 */
export function cssEntrypoint(config: BaseConfig): string {
  const entry = config.css ?? config.theme;
  if (!entry) {
    throw new Error("Stillwind requires a CSS-first entrypoint (`css`)");
  }
  return entry;
}

/**
 * Applies host defaults to settings passed to a native design-system loader.
 *
 * @param config - Shared compiler settings. Naming callbacks are passed separately to sessions.
 * @returns Native loader options. CSS evaluation supplies utility and variant definitions.
 */
export function compilerOptions(config: BaseConfig): NativeCompilerOptions {
  return {
    lint: config.lint && {
      noArbitraryColors: config.lint["no-arbitrary-colors"],
      noArbitrarySpacing:
        typeof config.lint["no-arbitrary-spacing"] === "string"
          ? {
              severity: config.lint["no-arbitrary-spacing"],
            }
          : config.lint["no-arbitrary-spacing"],
      noClassProperty: config.lint["no-class-property"],
      noRestrictedUtilities: config.lint["no-restricted-utilities"],
      noRestrictedDeclarations: config.lint["no-restricted-declarations"],
      noImportant: config.lint["no-important"],
      validLintSuppressions: config.lint["valid-lint-suppressions"],
      noUnusedLintSuppressions: config.lint["no-unused-lint-suppressions"],
      preferDirectStyling: config.lint["prefer-direct-styling"],
      duplicateUtilities: config.lint["duplicate-utilities"],
      conflictingUtilities:
        typeof config.lint["conflicting-utilities"] === "string"
          ? { severity: config.lint["conflicting-utilities"] }
          : config.lint["conflicting-utilities"],
      preferLogicalProperties: config.lint["prefer-logical-properties"],
      lengthUnits:
        typeof config.lint["length-units"] === "string"
          ? {
              severity: config.lint["length-units"],
            }
          : config.lint["length-units"],
      noRestrictedThemeTokens: config.lint["no-restricted-theme-tokens"],
      noArbitraryBreakpoints: config.lint["no-arbitrary-breakpoints"],
      canonicalUtilities:
        typeof config.lint["canonical-utilities"] === "string"
          ? {
              severity: config.lint["canonical-utilities"],
            }
          : config.lint["canonical-utilities"],
      sortUtilities: config.lint["sort-utilities"],
    },
    prefix: config.prefix ?? "--sw-",
    tailwindPrefix: config.tailwindPrefix ?? "--tw-",
    classNamespace: config.classNamespace ?? "",
    useCssIf: config.useCssIf ?? false,
    namingRevision: config.namingRevision ?? "",
    hashLength: config.hashLength,
  };
}
