//! Lint rule identities and reporting settings shared by producers and hosts.
//!
//! Parse rule names with [`LintKind::from_name`] and insert [`LintConfig`] values into
//! [`LintSettings`]. Unconfigured rules are off. Serialization emits the map from names
//! to severities that hosts use.
//!
//! Modules:
//!
//! - [`catalog`] declares stable rule names, default options, and the kinds of source
//!   that each rule checks.
//! - [`overrides`] merges host settings into the defaults.
//! - [`policies`] pairs detailed detection settings with their partial overrides.
//! - [`presets`] selects built-in rule severities.
//! - [`settings`] indexes the effective settings and looks them up for reporting.

mod catalog;
mod overrides;
mod policies;
mod presets;
mod settings;

pub use catalog::{LintCategory, LintConfig, LintKind, LintSeverity, LintSurface};
pub use overrides::LintOverrides;
pub use policies::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    ImportantConfig, ImportantOverride, LogicalPropertiesConfig, LogicalPropertiesOverride,
    MarkerUseConfig, MarkerUseOverride, RestrictedDeclarationRule, RestrictedDeclarationsConfig,
    RestrictedDeclarationsOverride, RestrictedLengthUnitRule, RestrictedLengthUnitsConfig,
    RestrictedLengthUnitsOverride, RestrictedThemeTokensConfig, RestrictedThemeTokensOverride,
    RestrictedUtilitiesConfig, RestrictedUtilitiesOverride, RestrictedUtilityRule,
    VariantDirectiveConfig, VariantDirectiveOverride,
};
pub use presets::LintPreset;
pub use settings::{DefinitionScope, LintSettings};
