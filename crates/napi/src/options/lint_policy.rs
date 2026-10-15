//! Converts host lint rule options into domain overrides.

use napi::Error;
use napi_derive::napi;

use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    LintSeverity, RestrictedDeclarationRule, RestrictedDeclarationsOverride,
    RestrictedLengthUnitRule, RestrictedLengthUnitsOverride, RestrictedThemeTokensOverride,
    RestrictedUtilitiesOverride, RestrictedUtilityRule,
};

/// Canonical utility options. All omitted fields preserve the previous setting.
#[napi(object)]
pub struct NativeCanonicalUtilitiesOptions {
    /// Reporting severity: warn, deny, or off.
    pub severity: Option<String>,
    /// Allows pixel spacing to use the active rem scale. Defaults to true.
    pub px_to_rem: Option<bool>,
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

/// Length restrictions applied at utility use sites.
#[napi(object)]
pub struct NativeRestrictedLengthUnitsOptions {
    /// Reporting severity; omission uses the rule default.
    pub severity: Option<String>,
    /// A supplied list replaces the existing restrictions.
    pub rules: Option<Vec<NativeRestrictedLengthUnitRule>>,
}

impl TryFrom<NativeRestrictedLengthUnitsOptions> for RestrictedLengthUnitsOverride {
    type Error = Error;

    fn try_from(value: NativeRestrictedLengthUnitsOptions) -> napi::Result<Self> {
        Ok(Self {
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

/// Forbidden direct theme token names.
#[napi(object)]
pub struct NativeRestrictedThemeTokensOptions {
    /// Reporting severity; omission uses the rule default.
    pub severity: Option<String>,
    /// Complete custom-property globs using `*` and `?`.
    pub tokens: Option<Vec<String>>,
}

impl TryFrom<NativeRestrictedThemeTokensOptions> for RestrictedThemeTokensOverride {
    type Error = Error;

    fn try_from(value: NativeRestrictedThemeTokensOptions) -> napi::Result<Self> {
        Ok(Self {
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

/// Restrictions on resolved CSS declarations.
#[napi(object)]
pub struct NativeRestrictedDeclarationsOptions {
    /// Reporting severity; omission uses the rule default.
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
                        properties: (rule.properties).into_iter().map(Into::into).collect(),
                        values: (rule.values).into_iter().map(Into::into).collect(),
                        message: (rule.message).map(Into::into),
                    })
                    .collect()
            }),
        })
    }
}

pub(super) fn lint_severity(value: &str) -> napi::Result<LintSeverity> {
    LintSeverity::parse(value)
        .ok_or_else(|| Error::from_reason("lint severity must be warn, deny, or off"))
}
