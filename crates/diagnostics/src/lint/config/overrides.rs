//! Applies explicit host fields to effective lint defaults without changing the inputs.

use serde::{Deserialize, Serialize};

use super::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    LintConfig, LintKind, LintSettings, LintSeverity, RestrictedDeclarationsConfig,
    RestrictedDeclarationsOverride, RestrictedLengthUnitsConfig, RestrictedLengthUnitsOverride,
    RestrictedThemeTokensConfig, RestrictedThemeTokensOverride, RestrictedUtilitiesConfig,
    RestrictedUtilitiesOverride,
};

/// Project lint settings applied to compiler defaults.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct LintOverrides {
    /// Reporting level for explicit color literals.
    pub no_arbitrary_colors: Option<LintSeverity>,
    /// Restrictions on explicit spacing lengths and percentages.
    pub no_arbitrary_spacing: Option<ArbitrarySpacingOverride>,
    /// Reporting level for external class contributions in style objects.
    pub no_class_property: Option<LintSeverity>,
    /// Forbidden utility spellings.
    pub no_restricted_utilities: Option<RestrictedUtilitiesOverride>,
    /// Forbidden resolved property and value combinations.
    pub no_restricted_declarations: Option<RestrictedDeclarationsOverride>,
    /// Reporting level for important declarations.
    pub no_important: Option<LintSeverity>,
    /// Reporting level for unknown suppression names and malformed directives.
    pub invalid_lint_suppression: Option<LintSeverity>,
    /// Reporting level for disable directives that suppress no enabled findings.
    pub unused_lint_suppression: Option<LintSeverity>,
    /// Whether variants must retain the element receiving the style.
    pub prefer_direct_styling: Option<LintSeverity>,
    /// Reporting level for repeated utility candidates.
    pub duplicate_utility: Option<LintSeverity>,
    /// Overlap policy for utilities supplied directly by one call.
    pub conflicting_utilities: Option<ConflictingUtilitiesOverride>,
    /// Reporting level for direction-dependent physical declarations.
    pub prefer_logical_properties: Option<LintSeverity>,
    /// Property-specific unit restrictions.
    pub no_restricted_length_units: Option<RestrictedLengthUnitsOverride>,
    /// Theme token names forbidden at their direct use sites.
    pub no_restricted_theme_tokens: Option<RestrictedThemeTokensOverride>,
    /// Reporting level for viewport size conditions supplied as arbitrary values.
    pub no_arbitrary_breakpoints: Option<LintSeverity>,
    /// Field-wise canonical policy overrides.
    pub prefer_canonical_utilities: Option<CanonicalUtilitiesOverride>,
    /// Optional unsorted-source diagnostic. It is disabled by default.
    pub prefer_sorted_utilities: Option<LintSeverity>,
    /// Reporting level for `mismatched-framework-import`.
    pub mismatched_framework_import: Option<LintSeverity>,
    /// Reporting level for `mutating-macro-argument`.
    pub mutating_macro_argument: Option<LintSeverity>,
    /// Reporting level for `suspending-macro-argument`.
    pub suspending_macro_argument: Option<LintSeverity>,
    /// Reporting level for omitted rule groups with incompatible inputs.
    pub invalid_rule_group_input: Option<LintSeverity>,
    /// Reporting level for `invalid-style-input`.
    pub invalid_style_input: Option<LintSeverity>,
    /// Reporting level for `invalid-cva-config`.
    pub invalid_cva_config: Option<LintSeverity>,
    /// Reporting level for `invalid-cva-input`.
    pub invalid_cva_input: Option<LintSeverity>,
    /// Reporting level for `invalid-keyframes-use`.
    pub invalid_keyframes_use: Option<LintSeverity>,
    /// Reporting level for `invalid-keyframes-definition`.
    pub invalid_keyframes_definition: Option<LintSeverity>,
    /// Reporting level for `invalid-keyframes-variant`.
    pub invalid_keyframes_variant: Option<LintSeverity>,
    /// Reporting level for `invalid-view-transition-use`.
    pub invalid_view_transition_use: Option<LintSeverity>,
    /// Reporting level for `invalid-view-transition-definition`.
    pub invalid_view_transition_definition: Option<LintSeverity>,
    /// Reporting level for `invalid-view-transition-variant`.
    pub invalid_view_transition_variant: Option<LintSeverity>,
    /// Reporting level for `invalid-ref-input`.
    pub invalid_ref_input: Option<LintSeverity>,
    /// Reporting level for `invalid-animation-option`.
    pub invalid_animation_option: Option<LintSeverity>,
    /// Reporting level for `invalid-style-composition`.
    pub invalid_style_composition: Option<LintSeverity>,
    /// Reporting level for `runtime-condition-requires-css-if`.
    pub runtime_condition_requires_css_if: Option<LintSeverity>,
    /// Reporting level for `unverified-style-base`.
    pub unverified_style_base: Option<LintSeverity>,
    /// Reporting level for `ineffective-anchor-scope`.
    pub ineffective_anchor_scope: Option<LintSeverity>,
    /// Reporting level for `missing-ref-attachment`.
    pub missing_ref_attachment: Option<LintSeverity>,
    /// Reporting level for `conflicting-provider-properties`.
    pub conflicting_provider_properties: Option<LintSeverity>,
    /// Reporting level for `prefer-typed-relationships`.
    pub prefer_typed_relationships: Option<LintSeverity>,
    /// Reporting level for `document-scoped-selector`.
    pub document_scoped_selector: Option<LintSeverity>,
    /// Reporting level for `repeated-nesting-selector`.
    pub repeated_nesting_selector: Option<LintSeverity>,
    /// Reporting level for `missing-view-transition-name`.
    pub missing_view_transition_name: Option<LintSeverity>,
    /// Reporting level for `duplicate-view-transition-name`.
    pub duplicate_view_transition_name: Option<LintSeverity>,
    /// Reporting level for `split-view-transition-class`.
    pub split_view_transition_class: Option<LintSeverity>,
    /// Reporting level for `ambiguous-style-base-order`.
    pub ambiguous_style_base_order: Option<LintSeverity>,
    /// Reporting level for `interfering-imported-style-base`.
    pub interfering_imported_style_base: Option<LintSeverity>,
    /// Reporting level for `invalid-utility`.
    pub invalid_utility: Option<LintSeverity>,
    /// Reporting level for `unknown-utility`.
    pub unknown_utility: Option<LintSeverity>,
    /// Reporting level for `invalid-variant`.
    pub invalid_variant: Option<LintSeverity>,
    /// Reporting level for `unknown-variant`.
    pub unknown_variant: Option<LintSeverity>,
    /// Reporting level for `invalid-property-value`.
    pub invalid_property_value: Option<LintSeverity>,
    /// Reporting level for `redundant-variant`.
    pub redundant_variant: Option<LintSeverity>,
    /// Reporting level for `unsatisfiable-variants`.
    pub unsatisfiable_variants: Option<LintSeverity>,
    /// Reporting level for `clamped-value`.
    pub clamped_value: Option<LintSeverity>,
    /// Reporting level for `prefer-typed-variables`.
    pub prefer_typed_variables: Option<LintSeverity>,
}

impl LintOverrides {
    /// Resolves project fields against the supplied defaults without changing either input.
    pub fn resolve(&self, defaults: &LintSettings) -> LintSettings {
        let mut settings = defaults.clone();
        self.resolve_reporting_levels(&mut settings);
        self.resolve_source_restrictions(&mut settings);
        if let Some(overrides) = &self.conflicting_utilities {
            let mut config = match settings.get(LintKind::ConflictingUtilities) {
                Some(LintConfig::ConflictingUtilities(config)) => config.clone(),
                _ => ConflictingUtilitiesConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.allow_shorthand_overrides {
                config.allow_shorthand_overrides = value;
            }
            settings.insert(LintConfig::ConflictingUtilities(config));
        }
        if let Some(overrides) = &self.no_restricted_length_units {
            let mut config = match settings.get(LintKind::NoRestrictedLengthUnits) {
                Some(LintConfig::NoRestrictedLengthUnits(config)) => config.clone(),
                _ => RestrictedLengthUnitsConfig {
                    severity: LintSeverity::Warn,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedLengthUnits(config));
        }
        if let Some(overrides) = &self.no_restricted_theme_tokens {
            let mut config = match settings.get(LintKind::NoRestrictedThemeTokens) {
                Some(LintConfig::NoRestrictedThemeTokens(config)) => config.clone(),
                _ => RestrictedThemeTokensConfig {
                    severity: LintSeverity::Warn,
                    tokens: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.tokens {
                config.tokens.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedThemeTokens(config));
        }
        if let Some(overrides) = &self.prefer_canonical_utilities {
            let mut config = match settings.get(LintKind::PreferCanonicalUtilities) {
                Some(LintConfig::PreferCanonicalUtilities(config)) => config.clone(),
                _ => CanonicalUtilitiesConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.px_to_rem {
                config.px_to_rem = value;
            }
            settings.insert(LintConfig::PreferCanonicalUtilities(config));
        }
        if let Some(severity) = self.prefer_sorted_utilities {
            settings.insert(LintConfig::PreferSortedUtilities(severity));
        }
        settings
    }

    fn resolve_reporting_levels(&self, settings: &mut LintSettings) {
        self.resolve_input_reporting_levels(settings);
        for (kind, severity) in [
            (
                LintKind::MismatchedFrameworkImport,
                self.mismatched_framework_import,
            ),
            (
                LintKind::MutatingMacroArgument,
                self.mutating_macro_argument,
            ),
            (
                LintKind::SuspendingMacroArgument,
                self.suspending_macro_argument,
            ),
            (
                LintKind::IneffectiveAnchorScope,
                self.ineffective_anchor_scope,
            ),
            (LintKind::MissingRefAttachment, self.missing_ref_attachment),
            (
                LintKind::ConflictingProviderProperties,
                self.conflicting_provider_properties,
            ),
            (
                LintKind::PreferTypedRelationships,
                self.prefer_typed_relationships,
            ),
            (
                LintKind::DocumentScopedSelector,
                self.document_scoped_selector,
            ),
            (
                LintKind::RepeatedNestingSelector,
                self.repeated_nesting_selector,
            ),
            (
                LintKind::MissingViewTransitionName,
                self.missing_view_transition_name,
            ),
            (
                LintKind::DuplicateViewTransitionName,
                self.duplicate_view_transition_name,
            ),
            (
                LintKind::SplitViewTransitionClass,
                self.split_view_transition_class,
            ),
            (
                LintKind::AmbiguousStyleBaseOrder,
                self.ambiguous_style_base_order,
            ),
            (
                LintKind::InterferingImportedStyleBase,
                self.interfering_imported_style_base,
            ),
            (LintKind::InvalidUtility, self.invalid_utility),
            (LintKind::UnknownUtility, self.unknown_utility),
            (LintKind::InvalidVariant, self.invalid_variant),
            (LintKind::UnknownVariant, self.unknown_variant),
            (LintKind::InvalidPropertyValue, self.invalid_property_value),
            (LintKind::RedundantVariant, self.redundant_variant),
            (LintKind::UnsatisfiableVariants, self.unsatisfiable_variants),
            (LintKind::ClampedValue, self.clamped_value),
            (LintKind::PreferTypedVariables, self.prefer_typed_variables),
            (LintKind::NoArbitraryColors, self.no_arbitrary_colors),
            (LintKind::NoClassProperty, self.no_class_property),
            (LintKind::NoImportant, self.no_important),
            (
                LintKind::InvalidLintSuppression,
                self.invalid_lint_suppression,
            ),
            (
                LintKind::UnusedLintSuppression,
                self.unused_lint_suppression,
            ),
            (LintKind::PreferDirectStyling, self.prefer_direct_styling),
            (LintKind::DuplicateUtility, self.duplicate_utility),
            (
                LintKind::NoArbitraryBreakpoints,
                self.no_arbitrary_breakpoints,
            ),
            (
                LintKind::PreferLogicalProperties,
                self.prefer_logical_properties,
            ),
        ] {
            if let Some(severity) = severity {
                settings.insert(LintConfig::new(kind, severity));
            }
        }
    }

    fn resolve_input_reporting_levels(&self, settings: &mut LintSettings) {
        for (kind, severity) in [
            (
                LintKind::InvalidRuleGroupInput,
                self.invalid_rule_group_input,
            ),
            (LintKind::InvalidStyleInput, self.invalid_style_input),
            (LintKind::InvalidCvaConfig, self.invalid_cva_config),
            (LintKind::InvalidCvaInput, self.invalid_cva_input),
            (LintKind::InvalidKeyframesUse, self.invalid_keyframes_use),
            (
                LintKind::InvalidKeyframesDefinition,
                self.invalid_keyframes_definition,
            ),
            (
                LintKind::InvalidKeyframesVariant,
                self.invalid_keyframes_variant,
            ),
            (
                LintKind::InvalidViewTransitionUse,
                self.invalid_view_transition_use,
            ),
            (
                LintKind::InvalidViewTransitionDefinition,
                self.invalid_view_transition_definition,
            ),
            (
                LintKind::InvalidViewTransitionVariant,
                self.invalid_view_transition_variant,
            ),
            (LintKind::InvalidRefInput, self.invalid_ref_input),
            (
                LintKind::InvalidAnimationOption,
                self.invalid_animation_option,
            ),
            (
                LintKind::InvalidStyleComposition,
                self.invalid_style_composition,
            ),
            (
                LintKind::RuntimeConditionRequiresCssIf,
                self.runtime_condition_requires_css_if,
            ),
            (LintKind::UnverifiedStyleBase, self.unverified_style_base),
        ] {
            if let Some(severity) = severity {
                settings.insert(LintConfig::new(kind, severity));
            }
        }
    }

    fn resolve_source_restrictions(&self, settings: &mut LintSettings) {
        if let Some(overrides) = &self.no_arbitrary_spacing {
            let mut config = match settings.get(LintKind::NoArbitrarySpacing) {
                Some(LintConfig::NoArbitrarySpacing(config)) => config.clone(),
                _ => ArbitrarySpacingConfig {
                    severity: LintSeverity::Warn,
                    properties: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.properties {
                config.properties.clone_from(value);
            }
            settings.insert(LintConfig::NoArbitrarySpacing(config));
        }
        if let Some(overrides) = &self.no_restricted_utilities {
            let mut config = match settings.get(LintKind::NoRestrictedUtilities) {
                Some(LintConfig::NoRestrictedUtilities(config)) => config.clone(),
                _ => RestrictedUtilitiesConfig {
                    severity: LintSeverity::Warn,
                    utilities: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.utilities {
                config.utilities.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedUtilities(config));
        }
        if let Some(overrides) = &self.no_restricted_declarations {
            let mut config = match settings.get(LintKind::NoRestrictedDeclarations) {
                Some(LintConfig::NoRestrictedDeclarations(config)) => config.clone(),
                _ => RestrictedDeclarationsConfig {
                    severity: LintSeverity::Warn,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedDeclarations(config));
        }
    }
}
