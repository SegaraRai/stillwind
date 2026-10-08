/**
 * Shared configuration entry point for build integrations and analysis hosts.
 * Define and evaluate configuration exports with defineConfig and resolveConfig.
 * Node discovery and file evaluation are available from @stillwind/config/node.
 */

export {
  normalizeCompilerConfig,
  validateCompilerConfig,
  type BaseConfig,
  type BuildConfig,
  type BuildOptions,
  type NormalizedCompilerConfig,
  type UtilitySortOptions,
} from "./compiler.ts";
export {
  defineConfig,
  resolveConfig,
  validateConfig,
  type AnalysisProjectConfig,
  type ProjectionProvider,
  type SourceProvider,
  type SourceProviderContext,
  type StillwindConfig,
  type StillwindConfigExport,
  type StillwindConfigFactory,
} from "./configuration.ts";
export { lintPresets } from "./lint-presets.ts";
export { lintRules, type LintRule } from "./lint-rules.ts";
export {
  mergeLintConfig,
  resolveLintConfig,
  type ArbitrarySpacingOptions,
  type CanonicalUtilitiesOptions,
  type ConflictingUtilitiesOptions,
  type LengthUnitsOptions,
  type LintConfig,
  type LintPreset,
  type LintRules,
  type LintSeverity,
  type ResolvedLintRules,
  type RestrictedDeclarationsOptions,
  type RestrictedThemeTokensOptions,
  type RestrictedUtilitiesOptions,
} from "./lint.ts";
export { mergeConfig } from "./merge.ts";
export type { InlineConfig, SerializableInlineConfig } from "./resolution.ts";
