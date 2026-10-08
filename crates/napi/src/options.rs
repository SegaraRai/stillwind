//! JavaScript-facing compiler options.
//!
//! Every field is optional and falls back to the Rust default, so a host can pass `{}`
//! and get the documented defaults. Values are validated here, which turns a bad prefix
//! or namespace into a JavaScript error before any source is read.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    LengthUnitRule, LengthUnitsOverride, LintOverrides, LintSeverity, RestrictedDeclarationRule,
    RestrictedDeclarationsOverride, RestrictedThemeTokensOverride, RestrictedUtilitiesOverride,
    RestrictedUtilityRule, RootFontSize,
};

fn lint_severity(value: &str) -> napi::Result<LintSeverity> {
    LintSeverity::from_css(value)
        .ok_or_else(|| Error::from_reason("lint severity must be warn, deny, or off"))
}

/// Canonical utility options. All omitted fields preserve the previous setting.
#[napi(object)]
pub struct NativeCanonicalUtilitiesOptions {
    /// Reporting severity: warn, deny, or off.
    pub severity: Option<String>,
    /// Allows pixel spacing to use the active rem scale. Defaults to true.
    pub px_to_rem: Option<bool>,
    /// Positive finite pixels per rem. Defaults to 16.
    pub root_font_size: Option<f64>,
}

/// Overlap policy for utilities supplied directly by one style call.
#[napi(object)]
pub struct NativeConflictingUtilitiesOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// Allows narrower declarations to override part of a shorthand.
    pub allow_shorthand_overrides: Option<bool>,
}

/// One property-specific length restriction; the last matching rule wins.
#[napi(object)]
pub struct NativeLengthUnitRule {
    /// Complete CSS property globs using `*` and `?`.
    pub properties: Vec<String>,
    /// Forbidden CSS length unit names.
    pub disallow: Vec<String>,
}

/// Length restrictions applied at utility use sites.
#[napi(object)]
pub struct NativeLengthUnitsOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// A supplied list replaces the existing restrictions.
    pub rules: Option<Vec<NativeLengthUnitRule>>,
}

/// Forbidden direct theme token names.
#[napi(object)]
pub struct NativeRestrictedThemeTokensOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// Complete custom-property globs using `*` and `?`.
    pub tokens: Option<Vec<String>>,
}

/// Arbitrary spacing restrictions, including optional additional property domains.
#[napi(object)]
pub struct NativeArbitrarySpacingOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// Additional complete CSS property globs using `*` and `?`.
    pub properties: Option<Vec<String>>,
}

impl TryFrom<NativeArbitrarySpacingOptions> for ArbitrarySpacingOverride {
    type Error = Error;

    fn try_from(value: NativeArbitrarySpacingOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            properties: value.properties,
        })
    }
}

/// A forbidden utility spelling and optional explanation.
#[napi(object)]
pub struct NativeRestrictedUtilityRule {
    /// Complete utility glob without variants or important modifiers.
    pub pattern: String,
    /// Optional project-specific explanation.
    pub message: Option<String>,
}

/// Restrictions on utility spellings.
#[napi(object)]
pub struct NativeRestrictedUtilitiesOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// A supplied list replaces utility restrictions.
    pub utilities: Option<Vec<NativeRestrictedUtilityRule>>,
}

impl TryFrom<NativeRestrictedUtilitiesOptions> for RestrictedUtilitiesOverride {
    type Error = Error;

    fn try_from(value: NativeRestrictedUtilitiesOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            utilities: value.utilities.map(|rules| {
                rules
                    .into_iter()
                    .map(|rule| RestrictedUtilityRule {
                        pattern: rule.pattern,
                        message: rule.message,
                    })
                    .collect()
            }),
        })
    }
}

/// A forbidden combination of CSS properties and values.
#[napi(object)]
pub struct NativeRestrictedDeclarationRule {
    /// Case-insensitive complete property globs.
    pub properties: Vec<String>,
    /// Case-insensitive complete serialized value globs.
    pub values: Vec<String>,
    /// Optional project-specific explanation.
    pub message: Option<String>,
}

/// Restrictions on resolved CSS declarations.
#[napi(object)]
pub struct NativeRestrictedDeclarationsOptions {
    /// Reporting severity; omission preserves the stylesheet level.
    pub severity: Option<String>,
    /// A supplied list replaces declaration restrictions.
    pub rules: Option<Vec<NativeRestrictedDeclarationRule>>,
}

impl TryFrom<NativeRestrictedDeclarationsOptions> for RestrictedDeclarationsOverride {
    type Error = Error;

    fn try_from(value: NativeRestrictedDeclarationsOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            rules: value.rules.map(|rules| {
                rules
                    .into_iter()
                    .map(|rule| RestrictedDeclarationRule {
                        properties: rule.properties,
                        values: rule.values,
                        message: rule.message,
                    })
                    .collect()
            }),
        })
    }
}

/// Project lint fields applied after CSS configuration.
#[napi(object)]
pub struct NativeLintOptions {
    /// Reporting level for arbitrary color literals. Defaults to off.
    pub no_arbitrary_colors: Option<String>,
    /// Restrictions on arbitrary spacing. Defaults to off.
    pub no_arbitrary_spacing: Option<NativeArbitrarySpacingOptions>,
    /// Reporting level for external class contributions. Defaults to off.
    pub no_class_property: Option<String>,
    /// Restrictions on utility spellings. Defaults to off.
    pub no_restricted_utilities: Option<NativeRestrictedUtilitiesOptions>,
    /// Restrictions on resolved declarations. Defaults to off.
    pub no_restricted_declarations: Option<NativeRestrictedDeclarationsOptions>,
    /// Reporting level for important declarations. Defaults to off.
    pub no_important: Option<String>,
    /// Reporting level for invalid suppression comments. Defaults to warn.
    pub valid_lint_suppressions: Option<String>,
    /// Reporting level for unused suppression comments. Defaults to off.
    pub no_unused_lint_suppressions: Option<String>,
    /// Reporting level for variants that select another target.
    pub prefer_direct_styling: Option<String>,
    /// Reporting level for repeated utility candidates.
    pub duplicate_utilities: Option<String>,
    /// Direct utility overlap policy.
    pub conflicting_utilities: Option<NativeConflictingUtilitiesOptions>,
    /// Reporting level for physical declarations with logical counterparts.
    pub prefer_logical_properties: Option<String>,
    /// Property-specific length unit restrictions.
    pub length_units: Option<NativeLengthUnitsOptions>,
    /// Forbidden direct theme token references.
    pub no_restricted_theme_tokens: Option<NativeRestrictedThemeTokensOptions>,
    /// Reporting level for arbitrary viewport size conditions. Defaults to off.
    pub no_arbitrary_breakpoints: Option<String>,
    /// Field-wise canonical lint policy overrides.
    pub canonical_utilities: Option<NativeCanonicalUtilitiesOptions>,
    /// Optional sorting diagnostic severity: warn, deny, or off. Detection is off by default.
    pub sort_utilities: Option<String>,
}

impl TryFrom<NativeLintOptions> for LintOverrides {
    type Error = Error;

    fn try_from(input: NativeLintOptions) -> napi::Result<Self> {
        let severity = |value: String| lint_severity(&value);
        Ok(Self {
            no_arbitrary_colors: input.no_arbitrary_colors.map(severity).transpose()?,
            no_class_property: input.no_class_property.map(severity).transpose()?,
            no_important: input.no_important.map(severity).transpose()?,
            valid_lint_suppressions: input.valid_lint_suppressions.map(severity).transpose()?,
            no_unused_lint_suppressions: input
                .no_unused_lint_suppressions
                .map(severity)
                .transpose()?,
            no_arbitrary_spacing: input
                .no_arbitrary_spacing
                .map(TryInto::try_into)
                .transpose()?,
            no_restricted_utilities: input
                .no_restricted_utilities
                .map(TryInto::try_into)
                .transpose()?,
            no_restricted_declarations: input
                .no_restricted_declarations
                .map(TryInto::try_into)
                .transpose()?,
            prefer_direct_styling: input.prefer_direct_styling.map(severity).transpose()?,
            duplicate_utilities: input.duplicate_utilities.map(severity).transpose()?,
            no_arbitrary_breakpoints: input.no_arbitrary_breakpoints.map(severity).transpose()?,
            prefer_logical_properties: input.prefer_logical_properties.map(severity).transpose()?,
            conflicting_utilities: input
                .conflicting_utilities
                .map(|value| {
                    Ok::<_, Error>(ConflictingUtilitiesOverride {
                        severity: value.severity.map(severity).transpose()?,
                        allow_shorthand_overrides: value.allow_shorthand_overrides,
                    })
                })
                .transpose()?,
            length_units: input
                .length_units
                .map(|value| {
                    Ok::<_, Error>(LengthUnitsOverride {
                        severity: value.severity.map(severity).transpose()?,
                        rules: value.rules.map(|rules| {
                            rules
                                .into_iter()
                                .map(|rule| LengthUnitRule {
                                    properties: rule.properties,
                                    disallow: rule.disallow,
                                })
                                .collect()
                        }),
                    })
                })
                .transpose()?,
            no_restricted_theme_tokens: input
                .no_restricted_theme_tokens
                .map(|value| {
                    Ok::<_, Error>(RestrictedThemeTokensOverride {
                        severity: value.severity.map(severity).transpose()?,
                        tokens: value.tokens,
                    })
                })
                .transpose()?,
            canonical_utilities: input
                .canonical_utilities
                .map(|config| {
                    Ok::<_, Error>(CanonicalUtilitiesOverride {
                        severity: config.severity.map(severity).transpose()?,
                        px_to_rem: config.px_to_rem,
                        root_font_size: config
                            .root_font_size
                            .map(|value| {
                                RootFontSize::new(value).ok_or_else(|| {
                                    Error::from_reason("rootFontSize must be positive and finite")
                                })
                            })
                            .transpose()?,
                    })
                })
                .transpose()?,
            sort_utilities: input.sort_utilities.map(severity).transpose()?,
        })
    }
}

/// Optional host overrides for prefixes, class identity, and conditional CSS.
/// Conversion validates prefixes; omitted fields retain [`CompilerOptions`] defaults.
#[napi(object)]
#[derive(Default)]
pub struct NativeCompilerOptions {
    /// Project lint overrides. Omitted fields retain stylesheet severity and canonical defaults.
    pub lint: Option<NativeLintOptions>,
    /// Prefix for Stillwind-owned identifiers. Must start with `--` and end with `-`.
    pub prefix: Option<String>,
    /// Prefix for the retained Tailwind-compatibility namespace.
    pub tailwind_prefix: Option<String>,
    /// Identifier namespace for generated class names.
    pub class_namespace: Option<String>,
    /// Emit native CSS `if()` for experimental conditional values and variants.
    /// Defaults to false; comparisons, negated variants, and CSS-wide helper branches
    /// require true.
    pub use_css_if: Option<bool>,
    /// Hash of the host's identifier generators, used to invalidate cached artifacts.
    pub naming_revision: Option<String>,
    /// Number of base-36 digits a built-in generated name keeps from its request hash.
    /// Defaults to 5. Values from 1 to 50 are accepted.
    pub hash_length: Option<u32>,
}

impl TryFrom<NativeCompilerOptions> for CompilerOptions {
    type Error = Error;

    fn try_from(input: NativeCompilerOptions) -> napi::Result<Self> {
        let defaults = Self::default();
        let options = Self {
            lint: input
                .lint
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or_default(),
            prefix: input.prefix.unwrap_or(defaults.prefix),
            tailwind_prefix: input.tailwind_prefix.unwrap_or(defaults.tailwind_prefix),
            class_namespace: input.class_namespace.unwrap_or(defaults.class_namespace),
            use_css_if: input.use_css_if.unwrap_or(defaults.use_css_if),
            naming_revision: input.naming_revision.unwrap_or(defaults.naming_revision),
            hash_length: input.hash_length.map_or(defaults.hash_length, |length| {
                usize::try_from(length).unwrap_or(usize::MAX)
            }),
        };
        options
            .validate()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(options)
    }
}

/// Resolves optional host overrides into validated compiler options.
pub(crate) fn compiler_options(
    options: Option<NativeCompilerOptions>,
) -> napi::Result<CompilerOptions> {
    options.unwrap_or_default().try_into()
}
