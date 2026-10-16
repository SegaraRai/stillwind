//! Stable lint identities, reporting severities, and default detection options.

use serde::{Deserialize, Serialize};

use super::{
    ArbitrarySpacingConfig, CanonicalUtilitiesConfig, ConflictingUtilitiesConfig,
    RestrictedDeclarationsConfig, RestrictedLengthUnitsConfig, RestrictedThemeTokensConfig,
    RestrictedUtilitiesConfig, VariantDirectiveConfig,
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

/// Kind of problem a lint rule reports, which tells the reader how to respond.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintCategory {
    /// Input whose contribution lowers as empty or whose condition lowers as false.
    Input,
    /// A violation of the macro value-expression or import contract. Output is unchanged.
    Contract,
    /// Valid output that probably differs from what the author intended.
    Hazard,
    /// A project-specific prohibition. Built-in presets leave these rules off.
    Restriction,
    /// A preferred spelling or structure for equivalent output.
    Style,
    /// A malformed or ineffective `swlint-*` suppression directive.
    Suppression,
}

impl LintCategory {
    /// Returns the lowercase spelling used by configuration catalogs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Contract => "contract",
            Self::Hazard => "hazard",
            Self::Restriction => "restriction",
            Self::Style => "style",
            Self::Suppression => "suppression",
        }
    }
}

macro_rules! define_lint_rules {
    ($( $category:ident { $( $variant:ident => $name:literal, )+ } )+) => {
        /// A lint rule identified by one stable configuration and suppression name.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum LintKind {
            $($(
                #[doc = concat!("The `", $name, "` rule.")]
                #[serde(rename = $name)]
                $variant,
            )+)+
        }

        impl LintKind {
            /// Every supported lint rule, in declaration order.
            pub const ALL: &'static [Self] = &[$($(Self::$variant,)+)+];

            /// Returns the stable configuration, suppression, and host-protocol name.
            pub const fn name(self) -> &'static str {
                match self { $($(Self::$variant => $name,)+)+ }
            }

            /// Returns the kind of problem this rule reports.
            pub const fn category(self) -> LintCategory {
                match self { $($(Self::$variant => LintCategory::$category,)+)+ }
            }

            /// Parses an exact rule name; unknown names return `None`.
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($($name => Some(Self::$variant),)+)+ _ => None }
            }
        }

        /// Configuration for one lint rule, including its reporting severity.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum LintConfig {
            $($(
                #[doc = concat!("Configuration for `", $name, "`.")]
                $variant(#[doc = "Settings for detection and reporting of this rule."] lint_config_type!($variant)),
            )+)+
        }

        impl LintConfig {
            /// Constructs the setting variant corresponding to `kind` with `severity`.
            pub const fn new(kind: LintKind, severity: LintSeverity) -> Self {
                match kind { $($(LintKind::$variant => Self::$variant(lint_config_value!($variant, severity)),)+)+ }
            }

            /// Returns the rule this setting configures.
            pub const fn kind(&self) -> LintKind {
                match self { $($(Self::$variant(_) => LintKind::$variant,)+)+ }
            }

            /// Returns the configured reporting severity.
            pub const fn severity(&self) -> LintSeverity {
                match self { $($(Self::$variant(config) => lint_config_severity!($variant, config),)+)+ }
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
    (NoRestrictedLengthUnits) => {
        RestrictedLengthUnitsConfig
    };
    (NoRestrictedThemeTokens) => {
        RestrictedThemeTokensConfig
    };
    (PreferCanonicalUtilities) => {
        CanonicalUtilitiesConfig
    };
    (PreferVariantDirective) => {
        VariantDirectiveConfig
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
    (NoRestrictedLengthUnits, $severity:expr) => {
        RestrictedLengthUnitsConfig {
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
    (PreferCanonicalUtilities, $severity:expr) => {
        CanonicalUtilitiesConfig {
            severity: $severity,
            px_to_rem: true,
        }
    };
    (PreferVariantDirective, $severity:expr) => {
        VariantDirectiveConfig {
            severity: $severity,
            px_to_rem: false,
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
    (NoRestrictedLengthUnits, $config:expr) => {
        $config.severity
    };
    (NoRestrictedThemeTokens, $config:expr) => {
        $config.severity
    };
    (PreferCanonicalUtilities, $config:expr) => {
        $config.severity
    };
    (PreferVariantDirective, $config:expr) => {
        $config.severity
    };
    ($variant:ident, $config:expr) => {
        *$config
    };
}

define_lint_rules! {
    Input {
        InvalidRuleGroupInput => "invalid-rule-group-input",
        InvalidStyleInput => "invalid-style-input",
        InvalidCvaConfig => "invalid-cva-config",
        InvalidCvaInput => "invalid-cva-input",
        InvalidKeyframesUse => "invalid-keyframes-use",
        InvalidKeyframesDefinition => "invalid-keyframes-definition",
        InvalidKeyframesVariant => "invalid-keyframes-variant",
        InvalidViewTransitionUse => "invalid-view-transition-use",
        InvalidViewTransitionDefinition => "invalid-view-transition-definition",
        InvalidViewTransitionVariant => "invalid-view-transition-variant",
        InvalidRefInput => "invalid-ref-input",
        InvalidAnimationOption => "invalid-animation-option",
        InvalidStyleComposition => "invalid-style-composition",
        RuntimeConditionRequiresCssIf => "runtime-condition-requires-css-if",
        InvalidUtility => "invalid-utility",
        UnknownUtility => "unknown-utility",
        InvalidVariant => "invalid-variant",
        UnknownVariant => "unknown-variant",
        MissingDocumentTarget => "missing-document-target",
        InvalidThemeReference => "invalid-theme-reference",
    }
    Contract {
        MutatingMacroArgument => "mutating-macro-argument",
        SuspendingMacroArgument => "suspending-macro-argument",
        MismatchedFrameworkImport => "mismatched-framework-import",
    }
    Hazard {
        DuplicateUtility => "duplicate-utility",
        ConflictingUtilities => "conflicting-utilities",
        ConflictingProviderProperties => "conflicting-provider-properties",
        UnverifiedStyleBase => "unverified-style-base",
        AmbiguousStyleBaseOrder => "ambiguous-style-base-order",
        InterferingImportedStyleBase => "interfering-imported-style-base",
        MissingRefAttachment => "missing-ref-attachment",
        DocumentScopedSelector => "document-scoped-selector",
        RepeatedNestingSelector => "repeated-nesting-selector",
        MissingViewTransitionName => "missing-view-transition-name",
        DuplicateViewTransitionName => "duplicate-view-transition-name",
        SplitViewTransitionClass => "split-view-transition-class",
        IneffectiveAnchorScope => "ineffective-anchor-scope",
        InvalidPropertyValue => "invalid-property-value",
        UnsatisfiableVariants => "unsatisfiable-variants",
        ClampedValue => "clamped-value",
        DuplicateDefinition => "duplicate-definition",
        InvalidThemeValue => "invalid-theme-value",
    }
    Restriction {
        NoArbitraryColors => "no-arbitrary-colors",
        NoArbitrarySpacing => "no-arbitrary-spacing",
        NoClassProperty => "no-class-property",
        NoRestrictedUtilities => "no-restricted-utilities",
        NoRestrictedDeclarations => "no-restricted-declarations",
        NoImportant => "no-important",
        NoRestrictedLengthUnits => "no-restricted-length-units",
        NoRestrictedThemeTokens => "no-restricted-theme-tokens",
        NoArbitraryBreakpoints => "no-arbitrary-breakpoints",
    }
    Style {
        PreferDirectStyling => "prefer-direct-styling",
        PreferLogicalProperties => "prefer-logical-properties",
        PreferTypedRelationships => "prefer-typed-relationships",
        PreferCanonicalUtilities => "prefer-canonical-utilities",
        PreferSortedUtilities => "prefer-sorted-utilities",
        RedundantVariant => "redundant-variant",
        PreferTypedVariables => "prefer-typed-variables",
        ImplicitNestingSelector => "implicit-nesting-selector",
        PreferVariantDirective => "prefer-variant-directive",
    }
    Suppression {
        InvalidLintSuppression => "invalid-lint-suppression",
        UnusedLintSuppression => "unused-lint-suppression",
    }
}

/// Kind of source that a lint rule checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LintSurface {
    /// Script and framework modules that call Stillwind macros.
    Script,
    /// Stylesheets that the host marks for linting.
    Stylesheet,
}

impl LintKind {
    /// Returns whether this rule checks sources of `surface`.
    ///
    /// One severity setting configures a rule on every surface that it checks. A
    /// suppression comment that names a rule that does not check its source is invalid.
    pub const fn checks(self, surface: LintSurface) -> bool {
        match surface {
            LintSurface::Script => !matches!(
                self,
                Self::InvalidThemeReference
                    | Self::DuplicateDefinition
                    | Self::InvalidThemeValue
                    | Self::ImplicitNestingSelector
                    | Self::PreferVariantDirective
            ),
            LintSurface::Stylesheet => matches!(
                self,
                Self::InvalidUtility
                    | Self::UnknownUtility
                    | Self::InvalidVariant
                    | Self::UnknownVariant
                    | Self::DuplicateUtility
                    | Self::ConflictingUtilities
                    | Self::DocumentScopedSelector
                    | Self::RepeatedNestingSelector
                    | Self::InvalidPropertyValue
                    | Self::UnsatisfiableVariants
                    | Self::ClampedValue
                    | Self::PreferCanonicalUtilities
                    | Self::PreferSortedUtilities
                    | Self::RedundantVariant
                    | Self::PreferTypedVariables
                    | Self::InvalidThemeReference
                    | Self::DuplicateDefinition
                    | Self::InvalidThemeValue
                    | Self::ImplicitNestingSelector
                    | Self::PreferVariantDirective
                    | Self::InvalidLintSuppression
                    | Self::UnusedLintSuppression
            ),
        }
    }
}
