//! Detected lint causes and their mapping to named rules.
//!
//! [`LintDiagnostic`] retains the concrete values used in its message template.
//! [`UtilityFailure`] classifies utility syntax and selects the corresponding rule.
//! Source locations and reporting settings are supplied by the caller.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::ModuleId;

use super::LintKind;

/// The reason a style input cannot be placed in a rule group.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum VariantInputErrorKind {
    /// Element attribute supplied by an object entry.
    #[error("element attribute")]
    Attribute,
    /// Inline custom property supplied by an object entry.
    #[error("inline custom property")]
    CustomProperty,
    /// Runtime class value without a CSS rule.
    #[error("external class value")]
    ExternalClass,
    /// A composed style value.
    #[error("style composition")]
    Composition,
    /// A ref attachment.
    #[error("ref attachment")]
    RefAttachment,
    /// A dynamic ref attachment.
    #[error("dynamic ref attachment")]
    DynamicRefAttachment,
}

/// Concrete values and message templates for a detected lint.
///
/// Distinct diagnostic variants may select the same rule. Utility classification can
/// select different rules for one variant.
/// Producers select the cause; hosts decide its presentation and configured severity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum LintDiagnostic {
    /// A projection receiver is not an identity created by a ref declaration.
    #[error("ref projections must use an identity returned by style.ref()")]
    InvalidRefProjection,
    /// A ref condition resolves to an empty set of ref declarations. Its condition is false.
    #[error("ref condition resolves to no style.ref() declaration")]
    EmptyRefCondition,
    /// A ref variant receiver resolves to a value that is not a ref declaration.
    #[error("ref variant receivers must resolve to a style.ref() declaration")]
    NonRefVariantReceiver,
    /// A computed key or scope operand is a bare value where a ref variant is required.
    #[error(
        "ref conditions must call a style.ref() declaration with a variant, such as ref(\"hover:\")"
    )]
    IncompleteRefCondition,
    /// A resolved variant cannot be expressed as a condition on the referenced element.
    #[error("variant cannot be applied as a ref condition")]
    UnsupportedRefVariant,
    /// A composition base is an inline macro call instead of a bound style value.
    #[error("nested style() calls are not supported; bind the base to a variable")]
    NestedStyleCall,
    /// A style input array contains a spread that cannot be statically expanded.
    #[error("style() array spreads require a literal array")]
    StyleArraySpread,
    /// An expression is outside the supported static style-input forms.
    #[error(
        "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and variant wrappers"
    )]
    InvalidStyleInput,
    /// A CVA configuration cannot be enumerated statically or contains invalid choices.
    #[error("invalid style.cva() configuration: {reason}")]
    InvalidCvaConfig {
        /// Concrete static syntax or selection requirement that failed.
        reason: LeanStr,
    },
    /// A CVA value is outside the supported utility and custom-property forms.
    #[error(
        "style.cva() values require static utilities, arrays, branches, CSS variants, or custom properties"
    )]
    InvalidCvaInput,
    /// An attribute object contains a spread whose properties cannot be resolved.
    #[error("style() object spreads require a literal object")]
    AttributeSpread,
    /// An attribute entry is not a plain key-value property.
    #[error("style() attributes must be plain key-value properties")]
    InvalidAttributeProperty,
    /// A computed attribute key cannot be resolved as a literal name or ref variant.
    #[error("computed style() keys must be literal strings or ref variants")]
    InvalidComputedAttribute,
    /// An object key is neither an attribute, a class entry, nor a variant prefix.
    #[error(
        "style() object keys must be `class`, quoted aria-*/data-* attributes, --* custom properties, `\"\"`, or variant prefixes ending in `:`"
    )]
    InvalidAttributeKey,
    /// An interpolation cannot be lowered as a complete ref-qualified utility segment.
    #[error("utility templates require direct ref variants in complete `variant:utility` segments")]
    InvalidUtilityTemplate,
    /// A style call contains a spread argument whose elements cannot be resolved.
    #[error("style() argument spreads require a literal array")]
    StyleArgumentSpread,
    /// A keyframes use receiver branch resolves to a value that is not keyframes.
    #[error("keyframes uses require a receiver returned by style.keyframes()")]
    InvalidKeyframesReceiver,
    /// A keyframes use does not supply one literal options string. The use has empty options.
    #[error("a keyframes use requires one literal options string")]
    InvalidKeyframesOptions,
    /// A keyframes declaration does not supply one static frame object.
    #[error("style.keyframes() requires one object of static keyframe utility strings")]
    InvalidKeyframesDefinition,
    /// A keyframes frame object contains a spread property.
    #[error("animation keyframes cannot use object spreads")]
    KeyframesSpread,
    /// A keyframes frame entry is not a plain static property.
    #[error("animation keyframes must be plain static properties")]
    InvalidKeyframesProperty,
    /// A frame key cannot be interpreted as a keyframe selector.
    #[error("keyframe selectors must be `from`, `to`, or percentage keys")]
    InvalidKeyframesFrameKey,
    /// A static frame selector contains an invalid percentage or selector list.
    #[error("keyframe selectors must be `from`, `to`, or comma-separated percentages")]
    InvalidKeyframesFrameSelector,
    /// A frame body is not a literal utility string.
    #[error("keyframe values must be literal utility strings")]
    InvalidKeyframesFrameValue,
    /// A keyframes label is not the declaration's own literal string.
    #[error("a keyframes label must be a literal string on the declaration itself")]
    InvalidKeyframesLabel,
    /// A keyed transition use supplies more than one key expression or a spread. The
    /// use has the empty key of `.for()`.
    #[error("viewTransition.for() accepts at most one key expression")]
    InvalidViewTransitionKeyExpression,
    /// A keyed transition use has a receiver that is known to be a different value.
    #[error("view transition uses require an identity returned by style.viewTransition()")]
    InvalidViewTransitionReceiver,
    /// A transition declaration does not supply one static definition object.
    #[error("style.viewTransition() requires one object of static utility strings")]
    InvalidViewTransitionDefinition,
    /// A transition definition contains a spread property.
    #[error("view transition definitions cannot use object spreads")]
    ViewTransitionSpread,
    /// A transition entry is not a plain static property.
    #[error("view transition definitions must be plain static properties")]
    InvalidViewTransitionProperty,
    /// A transition entry names an unsupported pseudo-element phase.
    #[error("view transition keys must be `group`, `old`, `new`, or `imagePair`")]
    InvalidViewTransitionKey,
    /// A transition body is not a literal utility string.
    #[error("view transition values must be literal utility strings")]
    InvalidViewTransitionValue,
    /// A view-transition label is not a literal string.
    #[error("a view transition label must be a literal string")]
    InvalidViewTransitionLabel,
    /// A ref label is not a literal string.
    #[error("style.ref() accepts only an optional literal label")]
    InvalidRefArgument,
    /// A ref declaration uses an unsupported argument count.
    #[error("style.ref() accepts () or (label)")]
    InvalidRefArguments,
    /// A ref variant's selector cannot be determined from literal text.
    #[error("ref variants require a static variant string")]
    NonStaticRefVariant,
    /// A shared scope call has an unsupported argument count or optional call.
    #[error("ref.in() requires exactly one ref or unscoped ref condition")]
    InvalidRefScopeArguments,
    /// A scope operand itself contains a shared scope call.
    #[error("an in() argument cannot contain in(); chain in() calls outside the argument")]
    NestedRefScope,
    /// A ref variant or container query is used as an unsupported receiver or operand.
    #[error(
        "variant receivers must be ref declarations; scope operands must be refs or unscoped ref variants"
    )]
    InvalidRefVariantReceiver,
    /// A container projection does not supply a literal container type. The projection
    /// has no container type.
    #[error("ref.asContainer() requires a valid literal CSS container type")]
    InvalidContainerType,
    /// A scope argument is not a keyword or an array, or an array entry is not a ref.
    /// An invalid argument is omitted, and an invalid entry contributes no identity.
    #[error("anchor scope requires all, none, or an array of finite ref candidates")]
    InvalidAnchorScope,
    /// A position-anchor use supplies arguments to a method that accepts none. The
    /// arguments are ignored.
    #[error("ref.positionAnchor() does not accept arguments")]
    InvalidPositionAnchorArguments,
    /// A runtime condition in a utility cannot be lowered without native CSS `if()`.
    /// The utility is omitted.
    #[error("`{utility}` cannot be lowered: {detail}")]
    RuntimeConditionRequiresCssIf {
        /// Utility text as authored, including variants.
        utility: LeanStr,
        /// The construct that the fallback strategy cannot represent.
        detail: LeanStr,
    },
    /// A composition base is a style result whose declaration is unknown. Its class is
    /// merged at runtime, but its CSS has no ordering dependency on the derived call.
    #[error("the style base cannot be identified, so its CSS may follow this call's CSS")]
    UnverifiedStyleBase,
    /// A composition base carries attributes or inline style. The base contributes only
    /// its class.
    #[error("a style base contributes only its class; its attributes and inline style are ignored")]
    StyleBaseWithProps,
    /// A composition base uses another framework's class property and contributes no
    /// class.
    #[error("a style base from another framework contributes no class")]
    StyleBaseFrameworkMismatch,
    /// A keyframe utility contains a variant forbidden in animation frames.
    #[error("animation keyframes cannot contain variants")]
    KeyframesFrameVariants,
    /// A definition-level variant cannot wrap a keyframes rule in valid CSS.
    #[error(
        "keyframes definition variants must be media or supports conditions; container queries do not condition global keyframes"
    )]
    InvalidKeyframesVariant,
    /// A view-transition utility carries a variant that cannot wrap a pseudo-element rule.
    #[error("view transition utilities accept only media and supports condition variants")]
    InvalidViewTransitionVariant,
    /// A variant prefix is syntactically malformed (`invalid-utility`).
    #[error("invalid variant prefix")]
    InvalidVariantPrefix,
    /// A syntactically valid variant prefix cannot be resolved in the theme (`unknown-utility`).
    #[error("unknown or unavailable variant prefix")]
    UnavailableVariantPrefix,
    /// An animation option token is not recognized by the option resolver.
    #[error("unknown animation option `{option}`")]
    UnknownAnimationOption {
        /// Name of the invalid configuration option.
        option: LeanStr,
    },
    /// A rule group is omitted because it contains an unsupported input.
    #[error("style() rule groups cannot contain {reason}")]
    InvalidVariantInput {
        /// First incompatible input in source order.
        reason: VariantInputErrorKind,
    },
    /// An explicitly supplied value contains a literal color.
    #[error(
        "arbitrary color `{value}` is forbidden for `{property}`; use a theme token or custom property"
    )]
    ArbitraryColor {
        /// Property receiving the source value.
        property: LeanStr,
        /// CSS text supplied by the source.
        value: LeanStr,
    },
    /// An explicitly supplied value contains a nonzero spacing measure.
    #[error(
        "arbitrary spacing `{value}` is forbidden for `{property}`; use the spacing scale or a custom property"
    )]
    ArbitrarySpacing {
        /// Property domain receiving the source value.
        property: LeanStr,
        /// CSS text supplied by the source.
        value: LeanStr,
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
        utility: LeanStr,
        /// Empty, or a colon followed by the configured explanation.
        explanation: LeanStr,
    },
    /// A resolved property and value match a project restriction.
    #[error("declaration `{property}: {value}` is forbidden{explanation}")]
    RestrictedDeclaration {
        /// Matched CSS property domain.
        property: LeanStr,
        /// Matched serialized value.
        value: LeanStr,
        /// Empty, or a colon followed by the configured explanation.
        explanation: LeanStr,
    },
    /// A resolved declaration has important priority.
    #[error("important declaration for `{property}` is forbidden")]
    ImportantDeclaration {
        /// CSS property receiving important priority.
        property: LeanStr,
    },
    /// A suppression directive contains an unknown name or invalid keyword.
    #[error("invalid lint suppression: {detail}")]
    InvalidLintSuppression {
        /// The unknown name or malformed directive.
        detail: LeanStr,
    },
    /// A disable directive did not suppress a finding from an enabled rule.
    #[error("lint suppression for `{rule}` suppresses no enabled findings")]
    UnusedLintSuppression {
        /// Named rule or `*` for all rules.
        rule: LeanStr,
    },
    /// A variant selects an element other than the attachment target.
    #[error(
        "variant selector `{selector}` styles another element. Attach these styles directly to that element"
    )]
    IndirectStyling {
        /// Expanded variant selector with `&` for the attachment element.
        selector: LeanStr,
    },
    /// A utility candidate is repeated under the same CSS conditions.
    #[error("utility `{utility}` is repeated under the same conditions")]
    DuplicateUtility {
        /// Resolved candidate spelling, including surrounding prefixes.
        utility: LeanStr,
    },
    /// Distinct utility candidates write competing declarations.
    #[error("utilities `{previous}` and `{utility}` overlap `{properties}`")]
    ConflictingUtilities {
        /// Candidate at the earlier source occurrence.
        previous: LeanStr,
        /// Candidate at the current source occurrence.
        utility: LeanStr,
        /// Competing property names and shorthand relationships.
        properties: LeanStr,
    },
    /// A physical declaration has a logical counterpart.
    #[error("prefer a logical declaration for `{property}: {value}`")]
    PhysicalDeclaration {
        /// Physical property name or property with a physical directional value.
        property: LeanStr,
        /// Serialized declaration value.
        value: LeanStr,
    },
    /// A nonzero length uses a forbidden unit in its property domain.
    #[error("unit `{unit}` is forbidden for `{property}`")]
    RestrictedLengthUnit {
        /// CSS property domain selecting the unit restriction.
        property: LeanStr,
        /// Lowercase CSS length unit.
        unit: LeanStr,
    },
    /// A direct reference names a forbidden theme token.
    #[error("direct use of theme token `{token}` is forbidden")]
    RestrictedThemeToken {
        /// Complete custom-property name directly selected by the utility.
        token: LeanStr,
    },
    /// A source variant supplies a viewport size condition without a named breakpoint.
    #[error("variant `{variant}` uses an arbitrary breakpoint `{condition}`; use a named variant")]
    ArbitraryBreakpoint {
        /// Source spelling of the variant that supplied the condition.
        variant: LeanStr,
        /// Expanded media query containing the viewport size condition.
        condition: LeanStr,
    },
    /// A source candidate has an equivalent canonical spelling.
    #[error("prefer canonical utility spelling")]
    NonCanonicalUtility,
    /// A static source container differs from compiler order.
    #[error("utilities are not in compiler order")]
    UnsortedUtilities,
    /// A value import of the style macro is incompatible with the original file format.
    #[error("`{import}` is incompatible with {source_format} input; use {expected}")]
    MismatchedFrameworkImport {
        /// Imported macro module specifier.
        import: LeanStr,
        /// Original format label, independent of the projected parser dialect.
        source_format: LeanStr,
        /// Allowed macro module specifiers joined with commas.
        expected: LeanStr,
    },
    /// A macro value expression visibly mutates state.
    #[error(
        "macro arguments must be pure; perform assignments, updates, and deletes before the macro call"
    )]
    MutatingMacroArgument,
    /// A macro expression suspends execution or loads a module; default severity is error.
    #[error(
        "evaluate await, yield, and dynamic imports outside macro arguments, then pass their values"
    )]
    SuspendingMacroArgument,
    /// An explicit anchor scope has no names or repeats an identity.
    #[error("anchor scope {detail}")]
    IneffectiveAnchorScope {
        /// Empty-list or repeated-identity explanation.
        detail: LeanStr,
    },
    /// A ref provider's style call supplies no matching marker attachment.
    #[error("ref `{name}` provider has no matching marker attachment in this style() call")]
    MissingRefAttachment {
        /// Ref binding displayed to the user.
        name: LeanStr,
    },
    /// A provider and another source supply overlapping CSS properties.
    #[error(
        "provider use overlaps `{properties}` from another source; their order is not guaranteed"
    )]
    ProviderConflict {
        /// Overlapping property names, including shorthand relationships.
        properties: LeanStr,
    },
    /// A Tailwind relationship utility violates the active typed-relationship policy.
    #[error(
        "Tailwind relationship utility `{utility}` is forbidden; use style.ref() and specify shared scopes with in()"
    )]
    TailwindRelationshipUtility {
        /// Utility text as authored, including variants.
        utility: LeanStr,
    },
    /// A style rule would apply to the document instead of to the styled element.
    #[error(
        "`{selector}` does not depend on the styled element; declare document-wide CSS in a global stylesheet"
    )]
    DocumentScopedSelector {
        /// Resolved selector that references neither the element nor a declaration identity.
        selector: LeanStr,
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
    SplitViewTransitionClass,
    /// Two composition bases declare one declaration slot and neither precedes the other.
    #[error(
        "two composition bases declare `{properties}`; neither base precedes the other, so which declaration wins is undefined"
    )]
    AmbiguousStyleBaseOrder {
        /// The competing declarations, as `property` or `property and property` when one
        /// side is a shorthand of the other.
        properties: LeanStr,
    },
    /// A base owned by another module declares a property this call also declares.
    #[error("the base in `{module}` overlaps these declarations: `{properties}`")]
    InterferingImportedStyleBase {
        /// Host identity of the module owning the base.
        module: ModuleId,
        /// The competing declarations, as `property` or `property and property`.
        properties: LeanStr,
    },
    /// A utility literal cannot be resolved and contributes nothing.
    #[error("unknown or unavailable utility `{utility}`")]
    UnresolvedUtility {
        /// Utility text as authored, including variants.
        utility: LeanStr,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
}

impl LintDiagnostic {
    /// Returns the rule selected by the diagnostic and its utility classification.
    pub const fn kind(&self) -> LintKind {
        match self {
            Self::EmptyRefCondition
            | Self::NonRefVariantReceiver
            | Self::IncompleteRefCondition
            | Self::UnsupportedRefVariant
            | Self::InvalidRefProjection
            | Self::InvalidRefArgument
            | Self::InvalidRefArguments
            | Self::NonStaticRefVariant
            | Self::InvalidRefScopeArguments
            | Self::NestedRefScope
            | Self::InvalidRefVariantReceiver
            | Self::InvalidContainerType
            | Self::InvalidAnchorScope
            | Self::InvalidPositionAnchorArguments => LintKind::InvalidRefInput,
            Self::NestedStyleCall
            | Self::StyleArraySpread
            | Self::InvalidStyleInput
            | Self::AttributeSpread
            | Self::InvalidAttributeProperty
            | Self::InvalidComputedAttribute
            | Self::InvalidAttributeKey
            | Self::InvalidUtilityTemplate
            | Self::StyleArgumentSpread => LintKind::InvalidStyleInput,
            Self::InvalidCvaConfig { .. } => LintKind::InvalidCvaConfig,
            Self::InvalidCvaInput => LintKind::InvalidCvaInput,
            Self::InvalidKeyframesOptions | Self::InvalidKeyframesReceiver => {
                LintKind::InvalidKeyframesUse
            }
            Self::InvalidKeyframesDefinition
            | Self::KeyframesSpread
            | Self::InvalidKeyframesProperty
            | Self::InvalidKeyframesFrameKey
            | Self::InvalidKeyframesFrameSelector
            | Self::InvalidKeyframesFrameValue
            | Self::InvalidKeyframesLabel => LintKind::InvalidKeyframesDefinition,
            Self::KeyframesFrameVariants | Self::InvalidKeyframesVariant => {
                LintKind::InvalidKeyframesVariant
            }
            Self::InvalidViewTransitionKeyExpression | Self::InvalidViewTransitionReceiver => {
                LintKind::InvalidViewTransitionUse
            }
            Self::InvalidViewTransitionDefinition
            | Self::ViewTransitionSpread
            | Self::InvalidViewTransitionProperty
            | Self::InvalidViewTransitionKey
            | Self::InvalidViewTransitionValue
            | Self::InvalidViewTransitionLabel => LintKind::InvalidViewTransitionDefinition,
            Self::InvalidViewTransitionVariant => LintKind::InvalidViewTransitionVariant,
            Self::InvalidVariantPrefix => LintKind::InvalidUtility,
            Self::UnavailableVariantPrefix => LintKind::UnknownUtility,
            Self::UnknownAnimationOption { .. } => LintKind::InvalidAnimationOption,
            Self::RuntimeConditionRequiresCssIf { .. } => LintKind::RuntimeConditionRequiresCssIf,
            Self::UnverifiedStyleBase => LintKind::UnverifiedStyleBase,
            Self::StyleBaseWithProps | Self::StyleBaseFrameworkMismatch => {
                LintKind::InvalidStyleComposition
            }
            Self::InvalidVariantInput { .. } => LintKind::InvalidVariantInput,
            Self::ArbitraryColor { .. } => LintKind::NoArbitraryColors,
            Self::ArbitrarySpacing { .. } => LintKind::NoArbitrarySpacing,
            Self::ClassProperty => LintKind::NoClassProperty,
            Self::RestrictedUtility { .. } => LintKind::NoRestrictedUtilities,
            Self::RestrictedDeclaration { .. } => LintKind::NoRestrictedDeclarations,
            Self::ImportantDeclaration { .. } => LintKind::NoImportant,
            Self::InvalidLintSuppression { .. } => LintKind::InvalidLintSuppression,
            Self::UnusedLintSuppression { .. } => LintKind::UnusedLintSuppression,
            Self::IndirectStyling { .. } => LintKind::PreferDirectStyling,
            Self::DuplicateUtility { .. } => LintKind::DuplicateUtility,
            Self::ConflictingUtilities { .. } => LintKind::ConflictingUtilities,
            Self::PhysicalDeclaration { .. } => LintKind::PreferLogicalProperties,
            Self::RestrictedLengthUnit { .. } => LintKind::NoRestrictedLengthUnits,
            Self::RestrictedThemeToken { .. } => LintKind::NoRestrictedThemeTokens,
            Self::ArbitraryBreakpoint { .. } => LintKind::NoArbitraryBreakpoints,
            Self::NonCanonicalUtility => LintKind::PreferCanonicalUtilities,
            Self::UnsortedUtilities => LintKind::PreferSortedUtilities,
            Self::MismatchedFrameworkImport { .. } => LintKind::MismatchedFrameworkImport,
            Self::MutatingMacroArgument => LintKind::MutatingMacroArgument,
            Self::SuspendingMacroArgument => LintKind::SuspendingMacroArgument,
            Self::IneffectiveAnchorScope { .. } => LintKind::IneffectiveAnchorScope,
            Self::MissingRefAttachment { .. } => LintKind::MissingRefAttachment,
            Self::ProviderConflict { .. } => LintKind::ConflictingProviderProperties,
            Self::TailwindRelationshipUtility { .. } => LintKind::PreferTypedRelationships,
            Self::DocumentScopedSelector { .. } => LintKind::DocumentScopedSelector,
            Self::MissingViewTransitionName => LintKind::MissingViewTransitionName,
            Self::DuplicateViewTransitionName { .. } => LintKind::DuplicateViewTransitionName,
            Self::SplitViewTransitionClass => LintKind::SplitViewTransitionClass,
            Self::AmbiguousStyleBaseOrder { .. } => LintKind::AmbiguousStyleBaseOrder,
            Self::InterferingImportedStyleBase { .. } => LintKind::InterferingImportedStyleBase,
            Self::UnresolvedUtility { classification, .. } => classification.lint_kind(),
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
    use lean_string::ToLeanString;

    use stillwind_source::SourceError;

    use super::*;
    use crate::{CompileDiagnostic, CompileErrorCode, SourceFailureContext};

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
        assert_eq!(findings[0].to_lean_string(), findings[1].to_lean_string());
        assert_eq!(
            findings[0].to_lean_string(),
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
            assert_eq!(error.to_lean_string(), "invalid source byte range 9..2");
        }
    }
}
