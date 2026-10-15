//! Compiler-wide configuration and the `--sw-` / `--tw-` prefix split.
//!
//! [`CompilerOptions`] travels with one compilation. It carries the Stillwind-owned
//! property prefix, the separately configurable Tailwind-compatibility prefix, the
//! caller's class namespace and the runtime-conditional output strategy.
//! [`CompilerOptions::resolved`] pins both prefixes to the snapshot's own
//! configuration, so a caller that passes stale options still compiles against the
//! theme it handed over. Every generated text that carries a prefix is substituted by
//! this module's `rewrite_generated`; the two namespaces must stay distinct, because a
//! consumer that recognizes one must not match the other.

use lean_string::{LeanStr, ToLeanStr};
use serde::{Deserialize, Serialize};

use stillwind_css::is_length_unit;
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot, UtilityOrderMode};
use stillwind_diagnostics::{CompileDiagnostic, LintConfig, LintKind, LintOverrides, LintSettings};
use stillwind_source::ReplaceText;

use crate::identifiers::{DEFAULT_HASH_LENGTH, MAXIMUM_HASH_LENGTH};

/// Knobs a host may set per compilation.
///
/// Serialized with camelCase fields and `#[serde(default)]`, so adding a field is
/// compatible with previously written configuration; defaults match the built-in
/// `stillwind` package and an unnamespaced project. Runtime-condition output strategy
/// applies to utilities and authored CSS, independently of design-system prefix
/// resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    /// Project lint settings applied to compiler defaults.
    pub lint: LintOverrides,
    /// Prefix for Stillwind-owned custom properties, for example `--sw-if-<hash>`.
    pub prefix: LeanStr,
    /// Prefix for internal custom properties kept for Tailwind compatibility.
    pub tailwind_prefix: LeanStr,
    /// Strategy for ordering resolved utilities. Defaults to authored definition positions.
    pub utility_order: UtilityOrderMode,
    /// Extra namespace mixed into class identity, so two independently built projects
    /// can share one design system without colliding on generated class names.
    pub class_namespace: LeanStr,
    /// Emit native CSS `if()` for experimental value and variant conditions.
    /// False (default) uses custom-property fallbacks; all conditional variants need
    /// `revert-rule`. Value comparisons, negated variants, and CSS-wide helper branches require true.
    pub use_css_if: bool,
    /// Revision of the identifier generators a host supplies for this compilation.
    ///
    /// A host that computes generated names in JavaScript passes a hash of its
    /// generator functions here, because their output is otherwise invisible to the
    /// artifact cache. It defaults to the empty string, which denotes the built-in
    /// generators, and carries no meaning for the compiler beyond its effect on the
    /// artifact key.
    pub naming_revision: LeanStr,
    /// Number of base-36 digits a built-in generated name keeps from its request hash.
    ///
    /// Every family shares the length. A shorter spelling is easier to read in
    /// generated markup, while a longer one leaves more room before two declarations
    /// can collide, so a host that generates many names raises it.
    pub hash_length: usize,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            lint: LintOverrides::default(),
            prefix: "--sw-".to_lean_str(),
            tailwind_prefix: "--tw-".to_lean_str(),
            utility_order: UtilityOrderMode::default(),
            class_namespace: LeanStr::new(),
            use_css_if: false,
            naming_revision: LeanStr::new(),
            hash_length: DEFAULT_HASH_LENGTH,
        }
    }
}

impl CompilerOptions {
    /// The design-system configuration these options imply, for snapshot construction.
    pub fn design_system(&self) -> DesignSystemOptions {
        DesignSystemOptions {
            prefix: self.prefix.clone(),
            tailwind_prefix: self.tailwind_prefix.clone(),
            utility_order: self.utility_order,
        }
    }

    /// Returns effective lint settings for this compilation without modifying the design system.
    pub fn lint_settings(&self) -> LintSettings {
        self.lint.resolve(&LintSettings::with_defaults())
    }

    /// Checks lint policies, prefix namespaces, and the generated-name hash length.
    ///
    /// # Errors
    ///
    /// Returns the typed prefix-shape or namespace-conflict failures from
    /// [`DesignSystemOptions::validate`], retaining the rejected configuration values,
    /// the rejected hash length when it is outside the range a digest can fill, or an
    /// invalid unit restriction or active color token policy.
    pub fn validate(&self) -> Result<(), CompileDiagnostic> {
        validate_lint_settings(&self.lint.resolve(&LintSettings::with_defaults()))?;
        self.design_system().validate()?;
        if !(1..=MAXIMUM_HASH_LENGTH).contains(&self.hash_length) {
            return Err(CompileDiagnostic::InvalidHashLength {
                length: self.hash_length,
                maximum: MAXIMUM_HASH_LENGTH,
            });
        }
        Ok(())
    }

    /// Returns the same options with prefixes and utility ordering taken from `snapshot`.
    ///
    /// Generated text must use the same namespaces and ordering as the snapshot used
    /// to resolve utilities.
    pub(crate) fn resolved(self, snapshot: &DesignSystemSnapshot) -> Self {
        Self {
            prefix: snapshot.config().prefix.clone(),
            tailwind_prefix: snapshot.config().tailwind_prefix.clone(),
            utility_order: snapshot.config().utility_order,
            ..self
        }
    }

    /// Substitutes the configured prefixes into generated text.
    ///
    /// Built-in declarations use the default prefixes. Private-use sentinels prevent
    /// a configured prefix that contains `--sw-` or `--tw-` from being rewritten again.
    pub(crate) fn rewrite_generated(&self, value: &(impl ToLeanStr + ?Sized)) -> LeanStr {
        let value = value.to_lean_str();
        if !value.contains('\u{e000}')
            && (self.prefix == "--sw-" || !value.contains("--sw-"))
            && (self.tailwind_prefix == "--tw-" || !value.contains("--tw-"))
        {
            return value;
        }
        value
            .replace_text("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace_text("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace_text("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace_text("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
            .into()
    }
}

pub(crate) fn validate_lint_settings(settings: &LintSettings) -> Result<(), CompileDiagnostic> {
    for config in settings.iter() {
        let detail = match config {
            LintConfig::NoArbitrarySpacing(config)
                if config
                    .properties
                    .iter()
                    .any(|p| p.is_empty() || p.chars().any(char::is_whitespace)) =>
            {
                Some("properties must contain nonempty CSS property patterns")
            }
            LintConfig::NoRestrictedUtilities(config)
                if config.utilities.iter().any(|rule| {
                    rule.pattern.is_empty() || rule.pattern.chars().any(char::is_whitespace)
                }) =>
            {
                Some("utilities require nonempty complete utility patterns")
            }
            LintConfig::NoRestrictedDeclarations(config)
                if config.rules.iter().any(|rule| {
                    rule.properties.is_empty()
                        || rule.values.is_empty()
                        || rule
                            .properties
                            .iter()
                            .any(|p| p.is_empty() || p.chars().any(char::is_whitespace))
                        || rule.values.iter().any(LeanStr::is_empty)
                }) =>
            {
                Some("rules require nonempty property and value patterns")
            }
            LintConfig::NoRestrictedThemeTokens(config)
                if settings.enabled_for_detection(LintKind::NoRestrictedThemeTokens)
                    && (config.tokens.is_empty()
                        || config.tokens.iter().any(|p| {
                            !p.starts_with("--")
                                || p.len() <= 2
                                || p.chars().any(char::is_whitespace)
                        })) =>
            {
                Some("tokens must contain nonempty custom-property name patterns starting with --")
            }
            LintConfig::NoRestrictedLengthUnits(config)
                if config.rules.iter().any(|rule| {
                    rule.properties.is_empty()
                        || rule
                            .properties
                            .iter()
                            .any(|p| p.is_empty() || p.chars().any(char::is_whitespace))
                        || rule.disallow.iter().any(|unit| !is_length_unit(unit))
                }) =>
            {
                Some("rules require property patterns and valid CSS length units")
            }
            _ => None,
        };
        if let Some(detail) = detail {
            return Err(CompileDiagnostic::InvalidLintConfiguration {
                rule: config.kind().name().into(),
                detail: detail.into(),
            });
        }
    }
    Ok(())
}
