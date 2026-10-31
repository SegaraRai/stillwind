//! Applies explicit host fields to effective lint defaults without changing the inputs.

use serde::{Deserialize, Serialize};

use super::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    DefinitionScope, ImportantConfig, ImportantOverride, LintConfig, LintKind, LintSettings,
    LintSeverity, LogicalPropertiesConfig, LogicalPropertiesOverride, MarkerUseConfig,
    MarkerUseOverride, RestrictedDeclarationsConfig, RestrictedDeclarationsOverride,
    RestrictedLengthUnitsConfig, RestrictedLengthUnitsOverride, RestrictedThemeTokensConfig,
    RestrictedThemeTokensOverride, RestrictedUtilitiesConfig, RestrictedUtilitiesOverride,
    VariantDirectiveConfig, VariantDirectiveOverride,
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
    /// Restrictions on important declarations and source modifiers.
    pub no_important: Option<ImportantOverride>,
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
    /// Checks of direction-dependent physical declarations.
    pub prefer_logical_properties: Option<LogicalPropertiesOverride>,
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
    /// Reporting level for `invalid-media-query`.
    pub invalid_media_query: Option<LintSeverity>,
    /// Reporting level for `invalid-css-input`.
    pub invalid_css_input: Option<LintSeverity>,
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
    /// Field-wise `invalid-marker-use` policy overrides.
    pub invalid_marker_use: Option<MarkerUseOverride>,
    /// Reporting level for `invalid-variant`.
    pub invalid_variant: Option<LintSeverity>,
    /// Reporting level for `unknown-variant`.
    pub unknown_variant: Option<LintSeverity>,
    /// Reporting level for `missing-document-target`.
    pub missing_document_target: Option<LintSeverity>,
    /// Reporting level for `incompatible-property-value`.
    pub incompatible_property_value: Option<LintSeverity>,
    /// Reporting level for `redundant-variant`.
    pub redundant_variant: Option<LintSeverity>,
    /// Reporting level for `unsatisfiable-variants`.
    pub unsatisfiable_variants: Option<LintSeverity>,
    /// Reporting level for `clamped-value`.
    pub clamped_value: Option<LintSeverity>,
    /// Reporting level for `prefer-typed-variables`.
    pub prefer_typed_variables: Option<LintSeverity>,
    /// Reporting level for `invalid-theme-reference`.
    pub invalid_theme_reference: Option<LintSeverity>,
    /// Reporting level for `invalid-css-import`.
    pub invalid_css_import: Option<LintSeverity>,
    /// Reporting level for `invalid-css-directive`.
    pub invalid_css_directive: Option<LintSeverity>,
    /// Reporting level for `invalid-runtime-condition`.
    pub invalid_runtime_condition: Option<LintSeverity>,
    /// Reporting level for `duplicate-definition`.
    pub duplicate_definition: Option<LintSeverity>,
    /// Reporting level for `ignored-definition-condition`.
    pub ignored_definition_condition: Option<LintSeverity>,
    /// Reporting level for `bypassed-undefinition`.
    pub bypassed_undefinition: Option<LintSeverity>,
    /// Reporting level for `incompatible-theme-value`.
    pub incompatible_theme_value: Option<LintSeverity>,
    /// Reporting level for `ignored-tailwind-feature`.
    pub ignored_tailwind_feature: Option<LintSeverity>,
    /// Reporting level for `implicit-nesting-selector`.
    pub implicit_nesting_selector: Option<LintSeverity>,
    /// Field-wise `prefer-variant-directive` policy overrides.
    pub prefer_variant_directive: Option<VariantDirectiveOverride>,
    /// Packages whose utility definitions lint checks.
    pub check_libraries: Option<DefinitionScope>,
    /// Plugins whose utility definitions lint checks.
    pub check_plugins: Option<DefinitionScope>,
}

impl LintOverrides {
    /// Returns effective settings by applying this override to `defaults`.
    ///
    /// Omitted fields preserve the existing policy. A detailed rule without a prior
    /// policy starts with its catalog defaults. Supplied lists replace existing lists.
    /// Neither input is changed.
    pub fn resolve(&self, defaults: &LintSettings) -> LintSettings {
        let mut settings = defaults.clone();
        if let Some(scope) = &self.check_libraries {
            settings.set_check_libraries(scope.clone());
        }
        if let Some(scope) = &self.check_plugins {
            settings.set_check_plugins(scope.clone());
        }
        self.resolve_reporting_levels(&mut settings);
        self.resolve_source_restrictions(&mut settings);
        if let Some(overrides) = &self.prefer_logical_properties {
            let mut config = match settings.get(LintKind::PreferLogicalProperties) {
                Some(LintConfig::PreferLogicalProperties(config)) => config.clone(),
                _ => LogicalPropertiesConfig {
                    severity: LintSeverity::Warn,
                    check_implementation: false,
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.check_implementation {
                config.check_implementation = value;
            }
            settings.insert(LintConfig::PreferLogicalProperties(config));
        }
        if let Some(overrides) = &self.invalid_marker_use {
            let mut config = match settings.get(LintKind::InvalidMarkerUse) {
                Some(LintConfig::InvalidMarkerUse(config)) => config.clone(),
                _ => MarkerUseConfig {
                    severity: LintSeverity::Warn,
                    ignored_classes: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.ignored_classes {
                config.ignored_classes.clone_from(value);
            }
            settings.insert(LintConfig::InvalidMarkerUse(config));
        }
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
                    check_implementation: false,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            if let Some(value) = overrides.check_implementation {
                config.check_implementation = value;
            }
            settings.insert(LintConfig::NoRestrictedLengthUnits(config));
        }
        if let Some(overrides) = &self.no_restricted_theme_tokens {
            let mut config = match settings.get(LintKind::NoRestrictedThemeTokens) {
                Some(LintConfig::NoRestrictedThemeTokens(config)) => config.clone(),
                _ => RestrictedThemeTokensConfig {
                    severity: LintSeverity::Warn,
                    check_implementation: true,
                    tokens: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.tokens {
                config.tokens.clone_from(value);
            }
            if let Some(value) = overrides.check_implementation {
                config.check_implementation = value;
            }
            settings.insert(LintConfig::NoRestrictedThemeTokens(config));
        }
        self.resolve_spelling_policies(&mut settings);
        if let Some(severity) = self.prefer_sorted_utilities {
            settings.insert(LintConfig::PreferSortedUtilities(severity));
        }
        settings
    }

    fn resolve_spelling_policies(&self, settings: &mut LintSettings) {
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
            if let Some(value) = overrides.collapse {
                config.collapse = value;
            }
            if let Some(value) = overrides.logical_to_physical {
                config.logical_to_physical = value;
            }
            if let Some(value) = overrides.preserve_specificity {
                config.preserve_specificity = value;
            }
            settings.insert(LintConfig::PreferCanonicalUtilities(config));
        }
        if let Some(overrides) = &self.prefer_variant_directive {
            let mut config = match settings.get(LintKind::PreferVariantDirective) {
                Some(LintConfig::PreferVariantDirective(config)) => config.clone(),
                _ => VariantDirectiveConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.px_to_rem {
                config.px_to_rem = value;
            }
            settings.insert(LintConfig::PreferVariantDirective(config));
        }
    }

    fn resolve_reporting_levels(&self, settings: &mut LintSettings) {
        self.resolve_input_and_definition_reporting_levels(settings);
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
            (
                LintKind::MissingDocumentTarget,
                self.missing_document_target,
            ),
            (
                LintKind::IncompatiblePropertyValue,
                self.incompatible_property_value,
            ),
            (LintKind::RedundantVariant, self.redundant_variant),
            (LintKind::UnsatisfiableVariants, self.unsatisfiable_variants),
            (LintKind::ClampedValue, self.clamped_value),
            (LintKind::PreferTypedVariables, self.prefer_typed_variables),
            (LintKind::NoArbitraryColors, self.no_arbitrary_colors),
            (LintKind::NoClassProperty, self.no_class_property),
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
        ] {
            if let Some(severity) = severity {
                settings.insert(LintConfig::new(kind, severity));
            }
        }
    }

    fn resolve_input_and_definition_reporting_levels(&self, settings: &mut LintSettings) {
        for (kind, severity) in [
            (
                LintKind::InvalidThemeReference,
                self.invalid_theme_reference,
            ),
            (LintKind::InvalidCssImport, self.invalid_css_import),
            (LintKind::InvalidCssDirective, self.invalid_css_directive),
            (
                LintKind::InvalidRuntimeCondition,
                self.invalid_runtime_condition,
            ),
            (LintKind::DuplicateDefinition, self.duplicate_definition),
            (LintKind::BypassedUndefinition, self.bypassed_undefinition),
            (
                LintKind::IgnoredDefinitionCondition,
                self.ignored_definition_condition,
            ),
            (
                LintKind::IncompatibleThemeValue,
                self.incompatible_theme_value,
            ),
            (
                LintKind::IgnoredTailwindFeature,
                self.ignored_tailwind_feature,
            ),
            (
                LintKind::ImplicitNestingSelector,
                self.implicit_nesting_selector,
            ),
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
            (LintKind::InvalidMediaQuery, self.invalid_media_query),
            (LintKind::InvalidCssInput, self.invalid_css_input),
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
        if let Some(overrides) = &self.no_important {
            let mut config = match settings.get(LintKind::NoImportant) {
                Some(LintConfig::NoImportant(config)) => config.clone(),
                _ => ImportantConfig {
                    severity: LintSeverity::Warn,
                    check_implementation: true,
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.check_implementation {
                config.check_implementation = value;
            }
            settings.insert(LintConfig::NoImportant(config));
        }
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
                    check_implementation: true,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            if let Some(value) = overrides.check_implementation {
                config.check_implementation = value;
            }
            settings.insert(LintConfig::NoRestrictedDeclarations(config));
        }
    }
}
