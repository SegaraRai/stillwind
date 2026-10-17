import {
  lintRules,
  normalizeCompilerConfig,
  type BaseConfig,
  type LintRule,
} from "@stillwind/config";
import type { NativeCompilerOptions } from "@stillwind/native";

/**
 * Selects the design-system stylesheet request.
 *
 * @param config - Compiler settings. css identifies the stylesheet.
 * @returns The authored path without resolving it. The caller applies the project root.
 * @throws If css does not supply a nonempty stylesheet path.
 */
export function cssEntrypoint(config: BaseConfig): string {
  const entry = config.css;
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
  const resolved = normalizeCompilerConfig(config);

  return {
    lint: Object.fromEntries(
      Object.entries(resolved.lint.rules).map(([name, setting]) => [
        lintRules[name as LintRule].nativeName,
        setting,
      ]),
    ),
    prefix: resolved.prefix,
    tailwindPrefix: resolved.tailwindPrefix,
    utilityOrder: resolved.utilityOrder,
    stylesheetSort: {
      applyUtilities: resolved.sort.applyUtilities,
      applyStatements: resolved.sort.applyStatements,
      variantBlocks: resolved.sort.variantBlocks,
    },
    target: resolved.target,
    classNamespace: resolved.classNamespace,
    useCssIf: resolved.useCssIf,
    namingRevision: resolved.namingRevision,
    hashLength: resolved.hashLength,
    rootFontSize: resolved.rootFontSize,
  };
}
