//! Stable lint identities, reporting severities, and rule-specific defaults.

use serde::{Deserialize, Serialize};

use super::{
    ArbitrarySpacingConfig, CanonicalUtilitiesConfig, ConflictingUtilitiesConfig,
    LengthUnitsConfig, RestrictedDeclarationsConfig, RestrictedThemeTokensConfig,
    RestrictedUtilitiesConfig, RootFontSize,
};

/// Configured reporting level of a lint finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Report a warning.
    Warn,
    /// Report an error without discarding suggested fixes.
    Deny,
    /// Omit the finding and its suggested fixes.
    Off,
}

impl LintSeverity {
    /// Returns the configuration and host-protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Off => "off",
        }
    }

    /// Parses an exact lint reporting level; unsupported values return `None`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "warn" => Some(Self::Warn),
            "deny" => Some(Self::Deny),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

macro_rules! define_lint_rules {
    ($( $variant:ident => ($name:literal, $default:expr), )+) => {
        /// A lint rule identified by one stable configuration and suppression name.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum LintKind {
            $(
                #[doc = concat!("The `", $name, "` rule.")]
                #[serde(rename = $name)]
                $variant,
            )+
        }

        impl LintKind {
            /// Every supported lint rule, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Returns the stable configuration, suppression, and host-protocol name.
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)+ }
            }

            /// Parses an exact rule name; unknown names return `None`.
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant),)+ _ => None }
            }

            /// Returns the default setting when detection is configured by default.
            /// Rules without a setting can still be reported when a producer detects them.
            pub const fn default_config(self) -> Option<LintConfig> {
                let severity: Option<LintSeverity> = match self {
                    $(Self::$variant => $default,)+
                };
                match severity {
                    Some(level) => Some(LintConfig::new(self, level)),
                    None => None,
                }
            }
        }

        /// Configuration for one lint rule, including its reporting severity.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum LintConfig {
            $(
                #[doc = concat!("Configuration for `", $name, "`.")]
                $variant(#[doc = "Settings for detection and reporting of this rule."] lint_config_type!($variant)),
            )+
        }

        impl LintConfig {
            /// Constructs the setting variant corresponding to `kind` with `severity`.
            pub const fn new(kind: LintKind, severity: LintSeverity) -> Self {
                match kind { $(LintKind::$variant => Self::$variant(lint_config_value!($variant, severity)),)+ }
            }

            /// Returns the rule this setting configures.
            pub const fn kind(&self) -> LintKind {
                match self { $(Self::$variant(_) => LintKind::$variant,)+ }
            }

            /// Returns the configured reporting severity.
            pub const fn severity(&self) -> LintSeverity {
                match self { $(Self::$variant(config) => lint_config_severity!($variant, config),)+ }
            }
        }
    };
}

macro_rules! lint_config_type {
    (NoArbitrarySpacing) => {
        ArbitrarySpacingConfig
    };
    (NoRestrictedUtilities) => {
        RestrictedUtilitiesConfig
    };
    (NoRestrictedDeclarations) => {
        RestrictedDeclarationsConfig
    };
    (ConflictingUtilities) => {
        ConflictingUtilitiesConfig
    };
    (LengthUnits) => {
        LengthUnitsConfig
    };
    (NoRestrictedThemeTokens) => {
        RestrictedThemeTokensConfig
    };
    (CanonicalUtilities) => {
        CanonicalUtilitiesConfig
    };
    ($variant:ident) => {
        LintSeverity
    };
}

macro_rules! lint_config_value {
    (NoArbitrarySpacing, $severity:expr) => {
        ArbitrarySpacingConfig {
            severity: $severity,
            properties: Vec::new(),
        }
    };
    (NoRestrictedUtilities, $severity:expr) => {
        RestrictedUtilitiesConfig {
            severity: $severity,
            utilities: Vec::new(),
        }
    };
    (NoRestrictedDeclarations, $severity:expr) => {
        RestrictedDeclarationsConfig {
            severity: $severity,
            rules: Vec::new(),
        }
    };
    (ConflictingUtilities, $severity:expr) => {
        ConflictingUtilitiesConfig {
            severity: $severity,
            allow_shorthand_overrides: false,
        }
    };
    (LengthUnits, $severity:expr) => {
        LengthUnitsConfig {
            severity: $severity,
            rules: Vec::new(),
        }
    };
    (NoRestrictedThemeTokens, $severity:expr) => {
        RestrictedThemeTokensConfig {
            severity: $severity,
            tokens: Vec::new(),
        }
    };
    (CanonicalUtilities, $severity:expr) => {
        CanonicalUtilitiesConfig {
            severity: $severity,
            px_to_rem: true,
            root_font_size: RootFontSize::DEFAULT,
        }
    };
    ($variant:ident, $severity:expr) => {
        $severity
    };
}

macro_rules! lint_config_severity {
    (NoArbitrarySpacing, $config:expr) => {
        $config.severity
    };
    (NoRestrictedUtilities, $config:expr) => {
        $config.severity
    };
    (NoRestrictedDeclarations, $config:expr) => {
        $config.severity
    };
    (ConflictingUtilities, $config:expr) => {
        $config.severity
    };
    (LengthUnits, $config:expr) => {
        $config.severity
    };
    (NoRestrictedThemeTokens, $config:expr) => {
        $config.severity
    };
    (CanonicalUtilities, $config:expr) => {
        $config.severity
    };
    ($variant:ident, $config:expr) => {
        *$config
    };
}

define_lint_rules! {
    NoArbitraryColors => ("no-arbitrary-colors", None),
    NoArbitrarySpacing => ("no-arbitrary-spacing", None),
    NoClassProperty => ("no-class-property", None),
    NoRestrictedUtilities => ("no-restricted-utilities", None),
    NoRestrictedDeclarations => ("no-restricted-declarations", None),
    NoImportant => ("no-important", None),
    ValidLintSuppressions => ("valid-lint-suppressions", Some(LintSeverity::Warn)),
    NoUnusedLintSuppressions => ("no-unused-lint-suppressions", None),
    PreferDirectStyling => ("prefer-direct-styling", None),
    DuplicateUtilities => ("duplicate-utilities", Some(LintSeverity::Warn)),
    ConflictingUtilities => ("conflicting-utilities", Some(LintSeverity::Warn)),
    PreferLogicalProperties => ("prefer-logical-properties", None),
    LengthUnits => ("length-units", None),
    NoRestrictedThemeTokens => ("no-restricted-theme-tokens", None),
    NoArbitraryBreakpoints => ("no-arbitrary-breakpoints", None),
    FrameworkImportMismatch => ("framework-import-mismatch", Some(LintSeverity::Warn)),
    MacroArgumentMutation => ("macro-argument-mutation", Some(LintSeverity::Deny)),
    MacroExecutionBoundary => ("macro-execution-boundary", Some(LintSeverity::Deny)),
    ValidAnchorScope => ("valid-anchor-scope", None),
    RequireRefAttachment => ("require-ref-attachment", None),
    ConflictingProviderProperties => ("conflicting-provider-properties", None),
    PreferTypedRelationships => ("prefer-typed-relationships", None),
    RequireElementScope => ("require-element-scope", Some(LintSeverity::Deny)),
    RequireViewTransitionName => ("require-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionName => ("single-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionClass => ("single-view-transition-class", Some(LintSeverity::Warn)),
    AmbiguousBaseOrder => ("ambiguous-base-order", Some(LintSeverity::Warn)),
    ImportedBaseInterference => ("imported-base-interference", Some(LintSeverity::Warn)),
    InvalidUtility => ("invalid-utility", None),
    UnknownUtility => ("unknown-utility", None),
    CanonicalUtilities => ("canonical-utilities", Some(LintSeverity::Warn)),
    SortUtilities => ("sort-utilities", None),
}
