//! Lint rule identities and reporting settings shared by producers and hosts.
//!
//! Parse rule names with [`LintKind::from_name`] and insert [`LintConfig`] values into
//! [`LintSettings`]. Configured detection and reporting of unconfigured findings use
//! separate lookups. Serialization emits the map of names to severities used by hosts.

//!
//! `catalog` declares stable rule names and defaults. `policies` pairs detailed
//! detection settings with their partial overrides. `overrides` merges host settings
//! into defaults. `settings` indexes effective settings and controls reporting lookup.

mod catalog;
mod overrides;
mod policies;
mod settings;

pub use catalog::{LintConfig, LintKind, LintSeverity};
pub use overrides::LintOverrides;
pub use policies::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    LengthUnitRule, LengthUnitsConfig, LengthUnitsOverride, RestrictedDeclarationRule,
    RestrictedDeclarationsConfig, RestrictedDeclarationsOverride, RestrictedThemeTokensConfig,
    RestrictedThemeTokensOverride, RestrictedUtilitiesConfig, RestrictedUtilitiesOverride,
    RestrictedUtilityRule, RootFontSize,
};
pub use settings::LintSettings;
