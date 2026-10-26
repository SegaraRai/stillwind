//! Converts host lint rule options into domain overrides.

use napi::Error;
use napi_derive::napi;

use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    ImportantOverride, LintSeverity, LogicalPropertiesOverride, MarkerUseOverride,
    RestrictedDeclarationRule, RestrictedDeclarationsOverride, RestrictedLengthUnitRule,
    RestrictedLengthUnitsOverride, RestrictedThemeTokensOverride, RestrictedUtilitiesOverride,
    RestrictedUtilityRule, VariantDirectiveOverride,
};

/// Canonical utility options. All omitted fields preserve the previous setting.
#[napi(object)]
pub struct NativeCanonicalUtilitiesOptions {
    /// Reporting severity: warn, deny, or off.
    pub severity: Option<String>,
    /// Whether a `px` length and a `rem` length are equal when they are equal at the
    /// root font size. The default is true.
    pub px_to_rem: Option<bool>,
    /// Whether several utilities of one class string may be replaced by one utility,
    /// such as `w-4 h-4` by `size-4`. The default is true.
    pub collapse: Option<bool>,
    /// Whether logical and physical properties may be replaced by one shorthand, such
    /// as `ms-4 pr-4` by `px-4`. The default is false.
    pub logical_to_physical: Option<bool>,
    /// Whether a replacement must keep the specificity of selectors. Set it to false to
    /// allow Tailwind's rewrites of ancestor selectors, such as `in-[p]` for
    /// `[p_&]:flex`. The default is true.
    pub preserve_specificity: Option<bool>,
}

impl TryFrom<NativeCanonicalUtilitiesOptions> for CanonicalUtilitiesOverride {
    type Error = Error;

    fn try_from(value: NativeCanonicalUtilitiesOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            px_to_rem: value.px_to_rem,
            collapse: value.collapse,
            logical_to_physical: value.logical_to_physical,
            preserve_specificity: value.preserve_specificity,
        })
    }
}

/// Options of `prefer-variant-directive`. All omitted fields preserve the previous
/// setting.
#[napi(object)]
pub struct NativeVariantDirectiveOptions {
    /// Reporting severity: warn, deny, or off.
    pub severity: Option<String>,
    /// Lets `px` lengths of `@media` conditions match `rem` variants at 16px = 1rem.
    /// Defaults to false.
    pub px_to_rem: Option<bool>,
}

impl TryFrom<NativeVariantDirectiveOptions> for VariantDirectiveOverride {
    type Error = Error;

    fn try_from(value: NativeVariantDirectiveOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            px_to_rem: value.px_to_rem,
        })
    }
}

/// Policy for candidates that cannot add the marker class that they name.
#[napi(object)]
pub struct NativeMarkerUseOptions {
    /// Reporting severity; omission uses the rule default.
    pub severity: Option<String>,
    /// Exact decoded marker class names whose findings are not reported.
    pub ignored_classes: Option<Vec<String>>,
}

impl TryFrom<NativeMarkerUseOptions> for MarkerUseOverride {
    type Error = Error;

    fn try_from(value: NativeMarkerUseOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            ignored_classes: (value.ignored_classes)
                .map(|values| values.into_iter().map(Into::into).collect()),
        })
    }
}

/// Overlap policy for utilities supplied directly by one style call.
#[napi(object)]
pub struct NativeConflictingUtilitiesOptions {
    /// Reporting severity; omission uses the rule default.
    pub severity: Option<String>,
    /// Allows narrower declarations to override part of a shorthand.
    pub allow_shorthand_overrides: Option<bool>,
}

impl TryFrom<NativeConflictingUtilitiesOptions> for ConflictingUtilitiesOverride {
    type Error = Error;

    fn try_from(value: NativeConflictingUtilitiesOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            allow_shorthand_overrides: value.allow_shorthand_overrides,
        })
    }
}

/// One property-specific length restriction; the last matching rule wins.
#[napi(object)]
pub struct NativeRestrictedLengthUnitRule {
    /// Complete CSS property globs using `*` and `?`.
    pub properties: Vec<String>,
    /// Forbidden CSS length unit names.
    pub disallow: Vec<String>,
}

/// Binding fields that override length restrictions at utility use sites.
#[napi(object)]
pub struct NativeRestrictedLengthUnitsOptions {
    /// Reporting severity. Omission preserves the existing severity, or uses warn.
    pub severity: Option<String>,
    /// Include fixed `(implementation)` declarations. Omission preserves the existing
    /// choice, or uses false. Captured candidate values remain checked.
    pub check_implementation: Option<bool>,
    /// A supplied list replaces the existing restrictions.
    pub rules: Option<Vec<NativeRestrictedLengthUnitRule>>,
}

impl TryFrom<NativeRestrictedLengthUnitsOptions> for RestrictedLengthUnitsOverride {
    type Error = Error;

    /// Converts `value` into a partial override, preserving omitted fields. Unit names
    /// and property patterns are copied without validation.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied severity is not `warn`, `deny`, or `off`.
    fn try_from(value: NativeRestrictedLengthUnitsOptions) -> napi::Result<Self> {
        Ok(Self {
            check_implementation: value.check_implementation,
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            rules: value.rules.map(|rules| {
                rules
                    .into_iter()
                    .map(|rule| RestrictedLengthUnitRule {
                        properties: (rule.properties).into_iter().map(Into::into).collect(),
                        disallow: (rule.disallow).into_iter().map(Into::into).collect(),
                    })
                    .collect()
            }),
        })
    }
}

/// Binding fields that override logical-property checks at utility use sites.
#[napi(object)]
pub struct NativeLogicalPropertiesOptions {
    /// Reporting severity. Omission preserves the existing severity, or uses warn.
    pub severity: Option<String>,
    /// Include fixed `(implementation)` declarations. Omission preserves the existing
    /// choice, or uses false. Captured candidate values remain checked.
    pub check_implementation: Option<bool>,
}

impl TryFrom<NativeLogicalPropertiesOptions> for LogicalPropertiesOverride {
    type Error = Error;

    /// Converts `value` into a partial override, preserving omitted fields.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied severity is not `warn`, `deny`, or `off`.
    fn try_from(value: NativeLogicalPropertiesOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            check_implementation: value.check_implementation,
        })
    }
}

/// Binding fields that override restrictions on direct theme token uses.
#[napi(object)]
pub struct NativeRestrictedThemeTokensOptions {
    /// Reporting severity. Omission preserves the existing severity, or uses warn.
    pub severity: Option<String>,
    /// Include token uses from fixed `(implementation)` declarations. Omission
    /// preserves the existing choice, or uses true. Tokens selected through
    /// placeholders or variant arguments remain checked.
    pub check_implementation: Option<bool>,
    /// Complete custom-property globs using `*` and `?`.
    pub tokens: Option<Vec<String>>,
}

impl TryFrom<NativeRestrictedThemeTokensOptions> for RestrictedThemeTokensOverride {
    type Error = Error;

    /// Converts `value` into a partial override, preserving omitted fields. Token
    /// patterns are copied without validation.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied severity is not `warn`, `deny`, or `off`.
    fn try_from(value: NativeRestrictedThemeTokensOptions) -> napi::Result<Self> {
        Ok(Self {
            check_implementation: value.check_implementation,
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            tokens: (value.tokens).map(|values| values.into_iter().map(Into::into).collect()),
        })
    }
}

/// Arbitrary spacing restrictions, including optional additional property domains.
#[napi(object)]
pub struct NativeArbitrarySpacingOptions {
    /// Reporting severity; omission uses the rule default.
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
            properties: (value.properties)
                .map(|values| values.into_iter().map(Into::into).collect()),
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
    /// Reporting severity; omission uses the rule default.
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
                        pattern: (rule.pattern).into(),
                        message: (rule.message).map(Into::into),
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

/// Binding fields that override restrictions on resolved CSS declarations.
#[napi(object)]
pub struct NativeRestrictedDeclarationsOptions {
    /// Reporting severity. Omission preserves the existing severity, or uses warn.
    pub severity: Option<String>,
    /// Include fixed `(implementation)` declarations. Omission preserves the existing
    /// choice, or uses true. Declarations with executable placeholders and unmarked
    /// extensions remain checked.
    pub check_implementation: Option<bool>,
    /// A supplied list replaces declaration restrictions.
    pub rules: Option<Vec<NativeRestrictedDeclarationRule>>,
}

impl TryFrom<NativeRestrictedDeclarationsOptions> for RestrictedDeclarationsOverride {
    type Error = Error;

    /// Converts `value` into a partial override, preserving omitted fields. Property
    /// patterns, value patterns, and messages are copied without validation.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied severity is not `warn`, `deny`, or `off`.
    fn try_from(value: NativeRestrictedDeclarationsOptions) -> napi::Result<Self> {
        Ok(Self {
            check_implementation: value.check_implementation,
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            rules: value.rules.map(|rules| {
                rules
                    .into_iter()
                    .map(|rule| RestrictedDeclarationRule {
                        properties: (rule.properties).into_iter().map(Into::into).collect(),
                        values: (rule.values).into_iter().map(Into::into).collect(),
                        message: (rule.message).map(Into::into),
                    })
                    .collect()
            }),
        })
    }
}

/// Binding fields that override importance restrictions on utility uses.
#[napi(object)]
pub struct NativeImportantOptions {
    /// Reporting severity. Omission preserves the existing severity, or uses warn.
    pub severity: Option<String>,
    /// Include fixed `(implementation)` declarations. Omission preserves the existing
    /// choice, or uses true. Source `!` modifiers and declarations with executable
    /// placeholders remain checked.
    pub check_implementation: Option<bool>,
}

impl TryFrom<NativeImportantOptions> for ImportantOverride {
    type Error = Error;

    /// Converts `value` into a partial override, preserving omitted fields.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied severity is not `warn`, `deny`, or `off`.
    fn try_from(value: NativeImportantOptions) -> napi::Result<Self> {
        Ok(Self {
            severity: value
                .severity
                .map(|value| lint_severity(&value))
                .transpose()?,
            check_implementation: value.check_implementation,
        })
    }
}

pub(super) fn lint_severity(value: &str) -> napi::Result<LintSeverity> {
    LintSeverity::parse(value)
        .ok_or_else(|| Error::from_reason("lint severity must be warn, deny, or off"))
}
