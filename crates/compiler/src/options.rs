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
use serde::{Deserialize, Deserializer, Serialize, de};

use stillwind_css::is_length_unit;
use stillwind_design_system::{
    BrowserTargets, DesignSystemOptions, DesignSystemSnapshot, UtilityOrderMode,
    rewrite_generated_prefixes,
};
use stillwind_diagnostics::{CompileDiagnostic, LintConfig, LintKind, LintOverrides, LintSettings};

use crate::identifiers::{DEFAULT_HASH_LENGTH, MAXIMUM_HASH_LENGTH};
use crate::query::StylesheetSortOptions;

/// The prefix of internal custom properties in the package's own CSS. Generated text
/// replaces it with `CompilerOptions::tailwind_prefix`.
pub(crate) const PACKAGE_TAILWIND_PREFIX: &str = "--tw-";

/// Knobs a host may set per compilation.
///
/// Serialized with camelCase fields and `#[serde(default)]`, so adding a field is
/// compatible with previously written configuration; defaults match the `stillwind`
/// package and an unnamespaced project. Runtime-condition output strategy applies to
/// utilities and authored CSS, independently of design-system prefix resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    /// Project lint settings applied to compiler defaults.
    pub lint: LintOverrides,
    /// Prefix for Stillwind-owned custom properties, for example `--sw-if-<hash>`.
    pub prefix: LeanStr,
    /// Prefix for internal custom properties kept for Tailwind compatibility.
    pub tailwind_prefix: LeanStr,
    /// Prefix that replaces the leading `--` of theme variables in generated CSS, or an
    /// empty string to keep their authored names.
    pub theme_prefix: LeanStr,
    /// Strategy for ordering resolved utilities. Defaults to authored definition
    /// positions.
    pub utility_order: UtilityOrderMode,
    /// Targets for stylesheet sorting and its diagnostics. These flags do not change
    /// generated CSS. Explicit sorting remains available when lint is disabled.
    pub stylesheet_sort: StylesheetSortOptions,
    /// Minimum browser versions used to remove generated color compatibility branches.
    pub target: BrowserTargets,
    /// Extra namespace mixed into class identity, so two independently built projects
    /// can share one design system without colliding on generated class names.
    pub class_namespace: LeanStr,
    /// Emit native CSS `if()` for experimental value and variant conditions.
    /// False (default) uses custom-property fallbacks; all conditional variants need
    /// `revert-rule`. Value comparisons, negated variants, and CSS-wide helper branches
    /// require true.
    pub use_css_if: bool,
    /// Revision of the identifier generators a host supplies for this compilation.
    ///
    /// A host that computes generated names in JavaScript passes a hash of its
    /// generator functions here, because their output is otherwise invisible to the
    /// artifact cache. It defaults to the empty string, which denotes the default
    /// generators, and carries no meaning for the compiler beyond its effect on the
    /// artifact key.
    pub naming_revision: LeanStr,
    /// Number of base-36 digits a default generated name keeps from its request hash.
    ///
    /// Every family shares the length. A shorter spelling is easier to read in
    /// generated markup, while a longer one leaves more room before two declarations
    /// can collide, so a host that generates many names raises it.
    pub hash_length: usize,
    /// Pixels per rem in the documents that use the generated CSS. Canonical lint
    /// converts pixel spacing with it, and editor previews annotate rem values with it.
    pub root_font_size: RootFontSize,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            lint: LintOverrides::default(),
            prefix: "--sw-".to_lean_str(),
            tailwind_prefix: PACKAGE_TAILWIND_PREFIX.to_lean_str(),
            theme_prefix: LeanStr::new(),
            utility_order: UtilityOrderMode::default(),
            stylesheet_sort: StylesheetSortOptions::default(),
            target: BrowserTargets::default(),
            class_namespace: LeanStr::new(),
            use_css_if: false,
            naming_revision: LeanStr::new(),
            hash_length: DEFAULT_HASH_LENGTH,
            root_font_size: RootFontSize::DEFAULT,
        }
    }
}

impl CompilerOptions {
    /// The design-system configuration these options imply, for snapshot construction.
    pub fn design_system(&self) -> DesignSystemOptions {
        DesignSystemOptions {
            prefix: self.prefix.clone(),
            tailwind_prefix: self.tailwind_prefix.clone(),
            theme_prefix: self.theme_prefix.clone(),
            utility_order: self.utility_order,
            target: self.target.clone(),
        }
    }

    /// Returns effective lint settings for this compilation without modifying the
    /// design system.
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

    /// Returns options with prefixes, utility ordering, and browser targets taken from
    /// `snapshot`.
    ///
    /// Generated text must use the same namespaces and ordering as the snapshot used
    /// to resolve utilities.
    pub(crate) fn resolved(self, snapshot: &DesignSystemSnapshot) -> Self {
        Self {
            prefix: snapshot.config().prefix.clone(),
            tailwind_prefix: snapshot.config().tailwind_prefix.clone(),
            theme_prefix: snapshot.config().theme_prefix.clone(),
            utility_order: snapshot.config().utility_order,
            target: snapshot.config().target.clone(),
            ..self
        }
    }

    /// Substitutes the configured prefixes into generated text.
    ///
    /// The package's declarations use the default prefixes. Private-use sentinels
    /// prevent a configured prefix that contains `--sw-` or `--tw-` from being
    /// rewritten again.
    pub(crate) fn rewrite_generated(&self, value: &(impl ToLeanStr + ?Sized)) -> LeanStr {
        rewrite_generated_prefixes(&self.prefix, &self.tailwind_prefix, &value.to_lean_str()).into()
    }
}

/// Positive finite number of CSS pixels per rem.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RootFontSize(f64);

impl RootFontSize {
    /// The initial font size of browsers, 16 pixels per rem.
    pub const DEFAULT: Self = Self(16.0);

    /// Returns `None` for zero, negative, or nonfinite values.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    /// Returns the validated number of pixels per rem.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for RootFontSize {}

impl Default for RootFontSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<'de> Deserialize<'de> for RootFontSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?)
            .ok_or_else(|| de::Error::custom("rootFontSize must be positive and finite"))
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
