//! Detected lint causes and their mapping to named rules.
//!
//! [`LintDiagnostic`] retains the concrete values used in its message template.
//! [`UtilityFailure`] classifies utility syntax and selects the corresponding rule.
//! Source locations and reporting settings are supplied by the caller.

use serde::{Deserialize, Serialize};

use stillwind_source::ModuleId;

use super::LintKind;

/// Concrete values and message templates for a detected lint.
///
/// Distinct diagnostic variants may select the same rule. Utility classification can
/// select different rules for one variant.
/// Producers select the cause; hosts decide its presentation and configured severity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum LintDiagnostic {
    /// An explicitly supplied value contains a literal color.
    #[error(
        "arbitrary color `{value}` is forbidden for `{property}`; use a theme token or custom property"
    )]
    ArbitraryColor {
        /// Property receiving the source value.
        property: String,
        /// CSS text supplied by the source.
        value: String,
    },
    /// An explicitly supplied value contains a nonzero spacing measure.
    #[error(
        "arbitrary spacing `{value}` is forbidden for `{property}`; use the spacing scale or a custom property"
    )]
    ArbitrarySpacing {
        /// Property domain receiving the source value.
        property: String,
        /// CSS text supplied by the source.
        value: String,
    },
    /// A style object passes through an external class value.
    #[error(
        "`class` contributions are forbidden in style objects; use utilities, custom properties, or state attributes"
    )]
    ClassProperty,
    /// A source utility matches a project restriction.
    #[error("utility `{utility}` is forbidden{explanation}")]
    RestrictedUtility {
        /// Utility spelling without enclosing variants or importance.
        utility: String,
        /// Empty, or a colon followed by the configured explanation.
        explanation: String,
    },
    /// A resolved property and value match a project restriction.
    #[error("declaration `{property}: {value}` is forbidden{explanation}")]
    RestrictedDeclaration {
        /// Matched CSS property domain.
        property: String,
        /// Matched serialized value.
        value: String,
        /// Empty, or a colon followed by the configured explanation.
        explanation: String,
    },
    /// A resolved declaration has important priority.
    #[error("important declaration for `{property}` is forbidden")]
    ImportantDeclaration {
        /// CSS property receiving important priority.
        property: String,
    },
    /// A suppression directive contains an unknown name or invalid keyword.
    #[error("invalid lint suppression: {detail}")]
    InvalidLintSuppression {
        /// The unknown name or malformed directive.
        detail: String,
    },
    /// A disable directive did not suppress a finding from an enabled rule.
    #[error("lint suppression for `{rule}` suppresses no enabled findings")]
    UnusedLintSuppression {
        /// Named rule or `*` for all rules.
        rule: String,
    },
    /// A variant selects an element other than the attachment target.
    #[error(
        "variant selector `{selector}` styles another element. Attach these styles directly to that element"
    )]
    IndirectStyling {
        /// Expanded variant selector with `&` for the attachment element.
        selector: String,
    },
    /// A utility candidate is repeated under the same CSS conditions.
    #[error("utility `{utility}` is repeated under the same conditions")]
    DuplicateUtility {
        /// Resolved candidate spelling, including surrounding prefixes.
        utility: String,
    },
    /// Distinct utility candidates write competing declarations.
    #[error("utilities `{previous}` and `{utility}` overlap `{properties}`")]
    ConflictingUtilities {
        /// Candidate at the earlier source occurrence.
        previous: String,
        /// Candidate at the current source occurrence.
        utility: String,
        /// Competing property names and shorthand relationships.
        properties: String,
    },
    /// A physical declaration has a logical counterpart.
    #[error("prefer a logical declaration for `{property}: {value}`")]
    PhysicalDeclaration {
        /// Physical property name or property with a physical directional value.
        property: String,
        /// Serialized declaration value.
        value: String,
    },
    /// A nonzero length uses a forbidden unit in its property domain.
    #[error("unit `{unit}` is forbidden for `{property}`")]
    ForbiddenLengthUnit {
        /// CSS property domain selecting the unit restriction.
        property: String,
        /// Lowercase CSS length unit.
        unit: String,
    },
    /// A direct reference names a forbidden theme token.
    #[error("direct use of theme token `{token}` is forbidden")]
    RestrictedThemeToken {
        /// Complete custom-property name directly selected by the utility.
        token: String,
    },
    /// A source variant supplies a viewport size condition without a named breakpoint.
    #[error("variant `{variant}` uses an arbitrary breakpoint `{condition}`; use a named variant")]
    ArbitraryBreakpoint {
        /// Source spelling of the variant that supplied the condition.
        variant: String,
        /// Expanded media query containing the viewport size condition.
        condition: String,
    },
    /// A source candidate has an equivalent canonical spelling.
    #[error("prefer canonical utility spelling")]
    CanonicalUtilities,
    /// A static source container differs from compiler order.
    #[error("utilities are not in compiler order")]
    SortUtilities,
    /// A value import of the style macro is incompatible with the original file format.
    #[error("`{import}` is incompatible with {source_format} input; use {expected}")]
    FrameworkImportMismatch {
        /// Imported macro module specifier.
        import: String,
        /// Original format label, independent of the projected parser dialect.
        source_format: String,
        /// Allowed macro module specifiers joined with commas.
        expected: String,
    },
    /// A macro value expression visibly mutates state.
    #[error(
        "macro arguments must be pure; perform assignments, updates, and deletes before the macro call"
    )]
    MacroArgumentMutation,
    /// A macro expression suspends execution or loads a module; default severity is error.
    #[error(
        "evaluate await, yield, and dynamic imports outside macro arguments, then pass their values"
    )]
    MacroExecutionBoundary,
    /// An explicit anchor scope has no names or repeats an identity.
    #[error("anchor scope {detail}")]
    AnchorScopeList {
        /// Empty-list or repeated-identity explanation.
        detail: String,
    },
    /// A ref provider's style call supplies no matching marker attachment.
    #[error("ref `{name}` provider has no matching marker attachment in this style() call")]
    MissingRefAttachment {
        /// Ref binding displayed to the user.
        name: String,
    },
    /// A provider and another source supply overlapping CSS properties.
    #[error(
        "provider use overlaps `{properties}` from another source; their order is not guaranteed"
    )]
    ProviderConflict {
        /// Overlapping property names, including shorthand relationships.
        properties: String,
    },
    /// A Tailwind relationship utility violates the active typed-relationship policy.
    #[error(
        "Tailwind relationship utility `{utility}` is forbidden; use style.ref() and specify shared scopes with in()"
    )]
    PreferTypedRelationships {
        /// Utility text as authored, including variants.
        utility: String,
    },
    /// A style rule would apply to the document instead of to the styled element.
    #[error(
        "`{selector}` does not depend on the styled element; declare document-wide CSS in a global stylesheet"
    )]
    DocumentScopedSelector {
        /// Resolved selector that references neither the element nor a declaration identity.
        selector: String,
    },
    /// A style call marks participants with a transition class but names none.
    #[error(
        "a view transition class without a name does not participate; add .for(key) or declare view-transition-name"
    )]
    MissingViewTransitionName,
    /// A style call supplies more than one view-transition-name for one element.
    #[error(
        "{count} view-transition-name sources in one style() call; only one element value applies, and a runtime name wins"
    )]
    DuplicateViewTransitionName {
        /// How many sources the call supplied.
        count: usize,
    },
    /// An element's view-transition classes come from more than one style call.
    #[error(
        "view-transition-class is one property: this call's list replaces the one its composed base declares, so declare the transitions in one call"
    )]
    SplitViewTransitionClasses,
    /// Two composition bases declare one declaration slot and neither precedes the other.
    #[error(
        "two composition bases declare `{properties}`; neither base precedes the other, so which declaration wins is undefined"
    )]
    AmbiguousBaseOrder {
        /// The competing declarations, as `property` or `property and property` when one
        /// side is a shorthand of the other.
        properties: String,
    },
    /// A base owned by another module declares a property this call also declares.
    #[error("the base in `{module}` overlaps these declarations: `{properties}`")]
    ImportedBaseInterference {
        /// Host identity of the module owning the base.
        module: ModuleId,
        /// The competing declarations, as `property` or `property and property`.
        properties: String,
    },
    /// A utility literal is retained as an external class because resolution failed.
    #[error("unknown or unavailable utility `{utility}`")]
    UnresolvedUtility {
        /// Utility text as authored, including variants.
        utility: String,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
    /// An editor candidate cannot be resolved against the pinned design system.
    #[error("unresolved candidate: {candidate}")]
    UnresolvedCandidate {
        /// Candidate text supplied to the query.
        candidate: String,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
    /// A candidate preview cannot be split into complete generated CSS blocks.
    #[error("invalid generated CSS block at byte {offset}")]
    InvalidCandidateCss {
        /// Zero-based UTF-8 byte offset within the offending generated CSS contribution.
        offset: usize,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
}

impl LintDiagnostic {
    /// Returns the rule selected by the diagnostic and its utility classification.
    pub const fn kind(&self) -> LintKind {
        match self {
            Self::ArbitraryColor { .. } => LintKind::NoArbitraryColors,
            Self::ArbitrarySpacing { .. } => LintKind::NoArbitrarySpacing,
            Self::ClassProperty => LintKind::NoClassProperty,
            Self::RestrictedUtility { .. } => LintKind::NoRestrictedUtilities,
            Self::RestrictedDeclaration { .. } => LintKind::NoRestrictedDeclarations,
            Self::ImportantDeclaration { .. } => LintKind::NoImportant,
            Self::InvalidLintSuppression { .. } => LintKind::ValidLintSuppressions,
            Self::UnusedLintSuppression { .. } => LintKind::NoUnusedLintSuppressions,
            Self::IndirectStyling { .. } => LintKind::PreferDirectStyling,
            Self::DuplicateUtility { .. } => LintKind::DuplicateUtilities,
            Self::ConflictingUtilities { .. } => LintKind::ConflictingUtilities,
            Self::PhysicalDeclaration { .. } => LintKind::PreferLogicalProperties,
            Self::ForbiddenLengthUnit { .. } => LintKind::LengthUnits,
            Self::RestrictedThemeToken { .. } => LintKind::NoRestrictedThemeTokens,
            Self::ArbitraryBreakpoint { .. } => LintKind::NoArbitraryBreakpoints,
            Self::CanonicalUtilities => LintKind::CanonicalUtilities,
            Self::SortUtilities => LintKind::SortUtilities,
            Self::FrameworkImportMismatch { .. } => LintKind::FrameworkImportMismatch,
            Self::MacroArgumentMutation => LintKind::MacroArgumentMutation,
            Self::MacroExecutionBoundary => LintKind::MacroExecutionBoundary,
            Self::AnchorScopeList { .. } => LintKind::ValidAnchorScope,
            Self::MissingRefAttachment { .. } => LintKind::RequireRefAttachment,
            Self::ProviderConflict { .. } => LintKind::ConflictingProviderProperties,
            Self::PreferTypedRelationships { .. } => LintKind::PreferTypedRelationships,
            Self::DocumentScopedSelector { .. } => LintKind::RequireElementScope,
            Self::MissingViewTransitionName => LintKind::RequireViewTransitionName,
            Self::DuplicateViewTransitionName { .. } => LintKind::SingleViewTransitionName,
            Self::SplitViewTransitionClasses => LintKind::SingleViewTransitionClass,
            Self::AmbiguousBaseOrder { .. } => LintKind::AmbiguousBaseOrder,
            Self::ImportedBaseInterference { .. } => LintKind::ImportedBaseInterference,
            Self::UnresolvedUtility { classification, .. }
            | Self::UnresolvedCandidate { classification, .. }
            | Self::InvalidCandidateCss { classification, .. } => classification.lint_kind(),
        }
    }

    /// Returns the stable lint name used in reports and suppression directives.
    pub const fn code(&self) -> &'static str {
        self.kind().name()
    }
}

/// Syntax classification of an unresolved utility or candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UtilityFailure {
    /// Candidate syntax is malformed (`invalid-utility`).
    Invalid,
    /// Syntax is valid, but no available utility resolves it (`unknown-utility`).
    Unknown,
}

impl UtilityFailure {
    /// Returns the lint rule for this syntax classification.
    pub const fn lint_kind(self) -> LintKind {
        match self {
            Self::Invalid => LintKind::InvalidUtility,
            Self::Unknown => LintKind::UnknownUtility,
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::SourceError;

    use super::*;
    use crate::{CompileDiagnostic, CompileErrorCode, SourceFailureContext};

    #[test]
    fn utility_diagnostics_share_rules_without_losing_their_contents() {
        for classification in [UtilityFailure::Invalid, UtilityFailure::Unknown] {
            let diagnostics = [
                LintDiagnostic::UnresolvedUtility {
                    utility: "x".into(),
                    classification,
                },
                LintDiagnostic::UnresolvedCandidate {
                    candidate: "x".into(),
                    classification,
                },
                LintDiagnostic::InvalidCandidateCss {
                    offset: 7,
                    classification,
                },
            ];
            let expected = match classification {
                UtilityFailure::Invalid => LintKind::InvalidUtility,
                UtilityFailure::Unknown => LintKind::UnknownUtility,
            };
            for diagnostic in &diagnostics {
                assert_eq!(diagnostic.kind(), expected);
                assert_eq!(diagnostic.code(), expected.name());
            }
            assert_ne!(diagnostics[0], diagnostics[1]);
            assert_ne!(diagnostics[1].to_string(), diagnostics[2].to_string());
        }
    }

    /// Utility classification chooses the lint rule while source context preserves the operation.
    #[test]
    fn one_message_template_can_report_multiple_codes() {
        let findings = [UtilityFailure::Invalid, UtilityFailure::Unknown].map(|classification| {
            LintDiagnostic::UnresolvedUtility {
                utility: "hover:[".into(),
                classification,
            }
        });
        assert_eq!(findings[0].code(), "invalid-utility");
        assert_eq!(findings[1].code(), "unknown-utility");
        assert_eq!(findings[0].to_string(), findings[1].to_string());
        assert_eq!(
            findings[0].to_string(),
            "unknown or unavailable utility `hover:[`"
        );

        for context in [
            SourceFailureContext::Utility,
            SourceFailureContext::Projection,
            SourceFailureContext::Stylesheet,
        ] {
            let error = CompileDiagnostic::source_failure(
                &SourceError::InvalidRange { start: 9, end: 2 },
                context,
            );
            assert_eq!(error.code(), CompileErrorCode::SourceInvalidRange);
            assert_eq!(
                error,
                CompileDiagnostic::SourceInvalidRange {
                    start: 9,
                    end: 2,
                    context
                }
            );
            assert_eq!(error.to_string(), "invalid source byte range 9..2");
        }
    }
}
