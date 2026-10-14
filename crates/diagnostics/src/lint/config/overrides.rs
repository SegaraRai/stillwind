//! Applies explicit host fields to effective lint defaults without changing the inputs.

use serde::{Deserialize, Serialize};

use super::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    LengthUnitsConfig, LengthUnitsOverride, LintConfig, LintKind, LintSettings, LintSeverity,
    RestrictedDeclarationsConfig, RestrictedDeclarationsOverride, RestrictedThemeTokensConfig,
    RestrictedThemeTokensOverride, RestrictedUtilitiesConfig, RestrictedUtilitiesOverride,
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
    pub valid_lint_suppressions: Option<LintSeverity>,
    /// Reporting level for disable directives that suppress no enabled findings.
    pub no_unused_lint_suppressions: Option<LintSeverity>,
    /// Whether variants must retain the element receiving the style.
    pub prefer_direct_styling: Option<LintSeverity>,
    /// Reporting level for repeated utility candidates.
    pub duplicate_utilities: Option<LintSeverity>,
    /// Overlap policy for utilities supplied directly by one call.
    pub conflicting_utilities: Option<ConflictingUtilitiesOverride>,
    /// Reporting level for direction-dependent physical declarations.
    pub prefer_logical_properties: Option<LintSeverity>,
    /// Property-specific unit restrictions.
    pub length_units: Option<LengthUnitsOverride>,
    /// Theme token names forbidden at their direct use sites.
    pub no_restricted_theme_tokens: Option<RestrictedThemeTokensOverride>,
    /// Reporting level for viewport size conditions supplied as arbitrary values.
    pub no_arbitrary_breakpoints: Option<LintSeverity>,
    /// Field-wise canonical policy overrides.
    pub canonical_utilities: Option<CanonicalUtilitiesOverride>,
    /// Optional unsorted-source diagnostic. It is disabled by default.
    pub sort_utilities: Option<LintSeverity>,
    /// Reporting level for `framework-import-mismatch`.
    pub framework_import_mismatch: Option<LintSeverity>,
    /// Reporting level for `macro-argument-mutation`.
    pub macro_argument_mutation: Option<LintSeverity>,
    /// Reporting level for `macro-execution-boundary`.
    pub macro_execution_boundary: Option<LintSeverity>,
    /// Reporting level for omitted rule groups with incompatible inputs.
    pub invalid_variant_input: Option<LintSeverity>,
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
    /// Reporting level for `runtime-condition-requires-native`.
    pub runtime_condition_requires_native: Option<LintSeverity>,
    /// Reporting level for `valid-anchor-scope`.
    pub valid_anchor_scope: Option<LintSeverity>,
    /// Reporting level for `require-ref-attachment`.
    pub require_ref_attachment: Option<LintSeverity>,
    /// Reporting level for `conflicting-provider-properties`.
    pub conflicting_provider_properties: Option<LintSeverity>,
    /// Reporting level for `prefer-typed-relationships`.
    pub prefer_typed_relationships: Option<LintSeverity>,
    /// Reporting level for `require-element-scope`.
    pub require_element_scope: Option<LintSeverity>,
    /// Reporting level for `require-view-transition-name`.
    pub require_view_transition_name: Option<LintSeverity>,
    /// Reporting level for `single-view-transition-name`.
    pub single_view_transition_name: Option<LintSeverity>,
    /// Reporting level for `single-view-transition-class`.
    pub single_view_transition_class: Option<LintSeverity>,
    /// Reporting level for `ambiguous-base-order`.
    pub ambiguous_base_order: Option<LintSeverity>,
    /// Reporting level for `imported-base-interference`.
    pub imported_base_interference: Option<LintSeverity>,
    /// Reporting level for `invalid-utility`.
    pub invalid_utility: Option<LintSeverity>,
    /// Reporting level for `unknown-utility`.
    pub unknown_utility: Option<LintSeverity>,
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
        if let Some(overrides) = &self.length_units {
            let mut config = match settings.get(LintKind::LengthUnits) {
                Some(LintConfig::LengthUnits(config)) => config.clone(),
                _ => LengthUnitsConfig {
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
            settings.insert(LintConfig::LengthUnits(config));
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
        if let Some(overrides) = &self.canonical_utilities {
            let mut config = match settings.get(LintKind::CanonicalUtilities) {
                Some(LintConfig::CanonicalUtilities(config)) => config.clone(),
                _ => CanonicalUtilitiesConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.px_to_rem {
                config.px_to_rem = value;
            }
            if let Some(value) = overrides.root_font_size {
                config.root_font_size = value;
            }
            settings.insert(LintConfig::CanonicalUtilities(config));
        }
        if let Some(severity) = self.sort_utilities {
            settings.insert(LintConfig::SortUtilities(severity));
        }
        settings
    }

    fn resolve_reporting_levels(&self, settings: &mut LintSettings) {
        self.resolve_input_reporting_levels(settings);
        for (kind, severity) in [
            (
                LintKind::FrameworkImportMismatch,
                self.framework_import_mismatch,
            ),
            (
                LintKind::MacroArgumentMutation,
                self.macro_argument_mutation,
            ),
            (
                LintKind::MacroExecutionBoundary,
                self.macro_execution_boundary,
            ),
            (LintKind::ValidAnchorScope, self.valid_anchor_scope),
            (LintKind::RequireRefAttachment, self.require_ref_attachment),
            (
                LintKind::ConflictingProviderProperties,
                self.conflicting_provider_properties,
            ),
            (
                LintKind::PreferTypedRelationships,
                self.prefer_typed_relationships,
            ),
            (LintKind::RequireElementScope, self.require_element_scope),
            (
                LintKind::RequireViewTransitionName,
                self.require_view_transition_name,
            ),
            (
                LintKind::SingleViewTransitionName,
                self.single_view_transition_name,
            ),
            (
                LintKind::SingleViewTransitionClass,
                self.single_view_transition_class,
            ),
            (LintKind::AmbiguousBaseOrder, self.ambiguous_base_order),
            (
                LintKind::ImportedBaseInterference,
                self.imported_base_interference,
            ),
            (LintKind::InvalidUtility, self.invalid_utility),
            (LintKind::UnknownUtility, self.unknown_utility),
            (LintKind::NoArbitraryColors, self.no_arbitrary_colors),
            (LintKind::NoClassProperty, self.no_class_property),
            (LintKind::NoImportant, self.no_important),
            (
                LintKind::ValidLintSuppressions,
                self.valid_lint_suppressions,
            ),
            (
                LintKind::NoUnusedLintSuppressions,
                self.no_unused_lint_suppressions,
            ),
            (LintKind::PreferDirectStyling, self.prefer_direct_styling),
            (LintKind::DuplicateUtilities, self.duplicate_utilities),
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
            (LintKind::InvalidVariantInput, self.invalid_variant_input),
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
                LintKind::RuntimeConditionRequiresNative,
                self.runtime_condition_requires_native,
            ),
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
