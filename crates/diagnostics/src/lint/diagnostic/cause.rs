//! Detected lint causes, their message templates, and the rule each cause selects.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::ModuleId;

use super::super::LintKind;
use super::candidate::{
    DidYouMean, DocumentScopedMessage, PropertyValueMessage, UtilityValueMessage,
};
use super::variables::VariableTypeMessage;
use super::variants::{RedundantVariantMessage, UnsatisfiableMessage, VariantDirectiveMessage};
use super::{MarkerUseErrorKind, RuleGroupInputErrorKind};

/// Concrete values and message templates for a detected lint.
///
/// Distinct diagnostic variants may select the same rule, and each variant selects one
/// rule. Producers select the cause; hosts decide its presentation and configured
/// severity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum LintDiagnostic {
    /// A projection receiver is not an identity created by a ref declaration.
    #[error("ref projections must use an identity returned by style.ref()")]
    InvalidRefProjection,
    /// A ref condition resolves to an empty set of ref declarations. Its condition is
    /// false.
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
    /// A macro call other than `style()` appears in a style input. The enclosing
    /// lowering would keep the call in JavaScript, so the smallest input that contains
    /// it is omitted.
    #[error(
        "{syntax} is not supported in the arguments of another macro call; bind its result to a variable"
    )]
    NestedMacroCall {
        /// Display spelling of the nested macro, such as `style.media()`.
        syntax: LeanStr,
    },
    /// A style input array contains a spread that cannot be statically expanded.
    #[error("style() array spreads require a literal array")]
    StyleArraySpread,
    /// An expression is outside the supported static style-input forms.
    #[error(
        "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, and variant wrappers"
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
        "style.cva() values require static utilities, arrays, CSS variants, or custom properties. Utility inputs cannot use && or ternary expressions"
    )]
    InvalidCvaInput,
    /// A style object contains a spread whose properties cannot be resolved.
    #[error("style() object spreads require a literal object")]
    StyleObjectSpread,
    /// A lookup in an array or object literal uses a table whose entries cannot be
    /// determined statically. The whole lookup is omitted.
    #[error(
        "indexed style() tables require literal spreads and static keys other than `__proto__`"
    )]
    InvalidStyleTable,
    /// An entry of an indexed table is not a plain property with a literal value. The
    /// entry is omitted, so the lookup selects nothing for its key.
    #[error(
        "indexed style() table values must be literal utility strings, null, undefined, or false"
    )]
    InvalidStyleTableEntry,
    /// A style object entry is not a plain key-value property.
    #[error("style() object entries must be plain key-value properties")]
    InvalidStyleProperty,
    /// A computed style key cannot be resolved as a literal name or ref variant.
    #[error("computed style() keys must be literal strings or ref variants")]
    InvalidComputedStyleKey,
    /// An object key is neither a class entry, a custom property, nor a variant prefix.
    #[error(
        "style() object keys must be `class`, --* custom properties, `\"\"`, or variant prefixes ending in `:`"
    )]
    InvalidStyleKey,
    /// An interpolation cannot be lowered as a complete ref-qualified utility segment.
    #[error("utility templates require direct ref variants in complete `variant:utility` segments")]
    InvalidUtilityTemplate,
    /// A style call contains a spread argument whose elements cannot be resolved.
    #[error("style() argument spreads require a literal array")]
    StyleArgumentSpread,
    /// A keyframes use receiver branch resolves to a value that is not keyframes.
    #[error("keyframes uses require a receiver returned by style.keyframes()")]
    InvalidKeyframesReceiver,
    /// A keyframes use does not supply one literal options string. The use has empty
    /// options.
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
    /// A composition base carries inline style. The base contributes only its class.
    #[error("a style base contributes only its class; its inline style is ignored")]
    StyleBaseWithInlineStyle,
    /// A keyframe utility contains a variant forbidden in animation frames.
    #[error("animation keyframes cannot contain variants")]
    KeyframesFrameVariants,
    /// A definition-level variant cannot wrap a keyframes rule in valid CSS.
    #[error(
        "keyframes definition variants must be media or supports conditions; container queries do not condition global keyframes"
    )]
    InvalidKeyframesVariant,
    /// A view-transition utility carries a variant that cannot wrap a pseudo-element
    /// rule.
    #[error("view transition utilities accept only media and supports condition variants")]
    InvalidViewTransitionVariant,
    /// A `style.media()` argument is not a static string of variants, or the call has
    /// no arguments. The argument matches no environment.
    #[error("style.media() requires string literals of variants: {reason}")]
    InvalidMediaArgument {
        /// What the argument or call supplies instead.
        reason: LeanStr,
    },
    /// A `style.css()` input is not a static utility string, array, or variant group.
    /// The input contributes no rules.
    #[error(
        "style.css() accepts only static utility strings, arrays, and variant groups: {reason}"
    )]
    InvalidCssInput {
        /// What the input supplies instead.
        reason: LeanStr,
    },
    /// A `style.css()` option is not a supported static value, or names a template that
    /// the stylesheet does not define. The option lowers as if it were omitted.
    #[error("invalid style.css() option: {reason}")]
    InvalidCssOption {
        /// The rejected option and the reason.
        reason: LeanStr,
    },
    /// A `style.media()` argument contains a variant that adds a condition other than a
    /// media query. The whole argument matches no environment.
    #[error("`{variant}` adds a condition that is not a media query")]
    NonMediaVariant {
        /// Variant text as written.
        variant: LeanStr,
    },
    /// A variant prefix of an object key or ref condition is not a list of complete
    /// `name:` segments (`invalid-variant`).
    #[error("variant prefixes must be complete `name:` segments")]
    InvalidVariantPrefix,
    /// No variant definition or theme token has the written name (`unknown-variant`).
    #[error("{detail}{}", DidYouMean(.suggestion))]
    UnknownVariant {
        /// Variant text that the finding's range covers.
        variant: LeanStr,
        /// Explanation that names the unknown text.
        detail: LeanStr,
        /// Complete spelling of the utility or variant prefix with the likely
        /// correction.
        suggestion: Option<LeanStr>,
    },
    /// A variant exists, but it rejects its value, its modifier, or its inner variant,
    /// or arbitrary variant text is not a valid selector (`invalid-variant`).
    #[error("{detail}")]
    InvalidVariant {
        /// Variant text that the finding's range covers.
        variant: LeanStr,
        /// Explanation that names the rejected part and the reason.
        detail: LeanStr,
    },
    /// A candidate of a document-level style does not select the element it styles,
    /// because no variant selects one or a variant refers to `&` before one does
    /// (`missing-document-target`).
    #[error("{detail}")]
    MissingDocumentTarget {
        /// Candidate or variant text that the finding's range covers.
        candidate: LeanStr,
        /// Explanation that names the variant that refers to `&` too early, if any.
        detail: LeanStr,
    },
    /// An option of a keyframes use is not a plain utility of the `animation` registry
    /// context.
    #[error("unknown animation option `{option}`")]
    UnknownAnimationOption {
        /// Option as written.
        option: LeanStr,
    },
    /// A rule group is omitted because it contains an unsupported input.
    #[error("style() rule groups cannot contain {reason}")]
    InvalidRuleGroupInput {
        /// First incompatible input in source order.
        reason: RuleGroupInputErrorKind,
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
        "`class` contributions are forbidden in style objects; use utilities, custom properties, or CSS variants"
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
    /// A source utility uses the important modifier.
    #[error("the `!` utility modifier is forbidden")]
    ImportantUtility,
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
    #[error("replace `{utility}` with `{replacement}`")]
    NonCanonicalUtility {
        /// The utility, or the space-separated utilities, as written in the source,
        /// without the quotes around them.
        utility: LeanStr,
        /// The canonical spelling that produces the same CSS, before escaping for the
        /// source string.
        replacement: LeanStr,
    },
    /// A static source container differs from compiler order.
    #[error("utilities are not in compiler order")]
    UnsortedUtilities,
    /// Authored CSS utility arguments or directive siblings differ from compiler order.
    #[error("Stillwind CSS inputs are not in compiler order")]
    UnsortedStylesheet,
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
    /// A macro expression suspends execution or loads a module; default severity is
    /// error.
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
    #[error("{}", DocumentScopedMessage { selector, branch: branch.as_deref() })]
    DocumentScopedSelector {
        /// Resolved selector that references neither the element nor a declaration
        /// identity.
        selector: LeanStr,
        /// The branch of a selector list that keeps the list document-wide, when the
        /// other branches could select the styled element on their own.
        branch: Option<LeanStr>,
    },
    /// A variant selector uses `&` more than once in one complex selector. Each `&`
    /// stands for a generated class, so the selector compares class identities instead
    /// of describing an element relationship.
    #[error(
        "`{selector}` uses `&` more than once, so it matches by generated class identity; relate elements with style.ref()"
    )]
    RepeatedNestingSelector {
        /// Resolved selector that contains the repeated `&`.
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
    /// Two composition bases declare one declaration slot and neither precedes the
    /// other.
    #[error(
        "two composition bases declare `{properties}`; neither base precedes the other, so which declaration wins is undefined"
    )]
    AmbiguousStyleBaseOrder {
        /// The competing declarations, as `property` or `property and property` when
        /// one side is a shorthand of the other.
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
    /// A candidate names a marker class in a form that cannot add the class, and no
    /// utility resolves the candidate (`invalid-marker-use`).
    #[error("`{utility}` cannot add the marker class `{class}` {reason}")]
    InvalidMarkerUse {
        /// Utility text as authored, including variants.
        utility: LeanStr,
        /// Marker class name that the candidate names.
        class: LeanStr,
        /// Form of the candidate that prevents the class from being added.
        reason: MarkerUseErrorKind,
    },
    /// No utility definition or theme token has the written name, so the utility
    /// contributes nothing (`unknown-utility`).
    #[error("{detail}{}", DidYouMean(.suggestion))]
    UnknownUtility {
        /// Utility text as authored, including variants.
        utility: LeanStr,
        /// Explanation that names the unknown text.
        detail: LeanStr,
        /// Utility text as authored with the likely correction applied.
        suggestion: Option<LeanStr>,
    },
    /// A utility is malformed, or an existing utility rejects its value form, modifier,
    /// or negative sign, so it contributes nothing (`invalid-utility`).
    #[error("{detail}")]
    InvalidUtility {
        /// Utility text as authored, including variants.
        utility: LeanStr,
        /// Explanation that names the rejected part and the reason.
        detail: LeanStr,
    },
    /// An arbitrary value that the grammar of its property rejects is emitted as
    /// written (`incompatible-property-value`).
    #[error("{}", PropertyValueMessage {
        utility,
        property,
        value,
        alternatives,
    })]
    IncompatiblePropertyValue {
        /// Utility text as authored, without enclosing variant wrappers.
        utility: LeanStr,
        /// Lowercase CSS property receiving the value.
        property: LeanStr,
        /// Decoded value text as emitted.
        value: LeanStr,
        /// The utility with a type hint for each value type it accepts, when `property`
        /// was chosen as a fallback because the value has no hint and its type cannot
        /// be inferred; empty otherwise.
        alternatives: Vec<LeanStr>,
    },
    /// No value type of a utility accepts an arbitrary value without a type hint, and
    /// no property grammar decides the value, so it is emitted as written
    /// (`incompatible-property-value`). The value reaches only custom properties or a
    /// declaration that contains `var()`, `env()`, or `attr()`.
    #[error("{}", UtilityValueMessage { utility, value, alternatives })]
    UnacceptedUtilityValue {
        /// Utility text as authored, without enclosing variant wrappers.
        utility: LeanStr,
        /// Decoded value text as emitted.
        value: LeanStr,
        /// The utility with a type hint for each value type it accepts, when it accepts
        /// several types; empty otherwise.
        alternatives: Vec<LeanStr>,
    },
    /// A `var()` value without a type hint refers to a variable of a known type that
    /// the receiving property rejects (`incompatible-property-value`).
    #[error("{}", VariableTypeMessage {
        utility,
        property,
        variable,
        value_type,
        namespace,
        alternatives,
    })]
    VariableTypeMismatch {
        /// Utility text as authored, without enclosing variant wrappers.
        utility: LeanStr,
        /// Lowercase CSS property receiving the variable.
        property: LeanStr,
        /// Custom-property name, including its leading `--`.
        variable: LeanStr,
        /// The variable's type as declared, such as `font-size` or `<length>`.
        value_type: LeanStr,
        /// Theme namespace that declares the type, such as `text`, or `None` when an
        /// `@property` rule registers it.
        namespace: Option<LeanStr>,
        /// The utility with each type hint that accepts the variable's type.
        alternatives: Vec<LeanStr>,
    },
    /// An opacity modifier gives `color-mix()` a percentage outside `0%` to `100%`; the
    /// declaration emits it as written (`incompatible-property-value`).
    #[error(
        "the opacity modifier `{modifier}` gives `color-mix()` `{percentage}`, but `color-mix()` accepts only percentages from 0% to 100%"
    )]
    OpacityModifierOutOfRange {
        /// Modifier text after the `/`, as written.
        modifier: LeanStr,
        /// Percentage that the declaration emits.
        percentage: LeanStr,
    },
    /// A variant has no effect because another variant of the candidate already
    /// requires its condition (`redundant-variant`).
    #[error("{}", RedundantVariantMessage { variant, kept, implied: *implied })]
    RedundantVariant {
        /// Text of the redundant variant, without its `:`.
        variant: LeanStr,
        /// Text of the variant that makes it redundant.
        kept: LeanStr,
        /// True when `kept` implies a weaker range; false when it repeats the variant.
        implied: bool,
    },
    /// Every branch of a variant stack's combined condition contains conditions that
    /// exclude each other, so the rule never matches (`unsatisfiable-variants`).
    #[error("{}", UnsatisfiableMessage { variants, first, second, branches: *branches })]
    UnsatisfiableVariants {
        /// Variant text from the first to the last contradicting variant.
        variants: LeanStr,
        /// Earlier of two conditions that exclude each other, as CSS text.
        first: LeanStr,
        /// Later of the two conditions, as CSS text.
        second: LeanStr,
        /// Number of alternative branches of the combined condition.
        branches: u32,
    },
    /// A literal value lies outside the range of its property or filter function, so
    /// browsers clamp it (`clamped-value`).
    #[error("browsers clamp `{value}` to `{clamped}`")]
    ClampedValue {
        /// The declaration or filter function with the written value, such as
        /// `opacity: 150%` or `grayscale(150%)`.
        value: LeanStr,
        /// The same text with the value that browsers use.
        clamped: LeanStr,
    },
    /// A `var()` value has no type hint although the utility accepts values of several
    /// types (`prefer-typed-variables`).
    #[error(
        "`{utility}` has no type hint, so it uses the `{hint}` definition; write `{replacement}` to state the type"
    )]
    UntypedVariable {
        /// Utility text as authored, without enclosing variant wrappers.
        utility: LeanStr,
        /// Type hint of the definition that receives the value.
        hint: LeanStr,
        /// The utility with the type hint added.
        replacement: LeanStr,
    },
    /// A selector of a variant definition has a branch without `&` or a branch that
    /// starts with a combinator, so it is relative to `&` without writing it
    /// (`implicit-nesting-selector`).
    #[error(
        "variant selector `{selector}` is relative to `&` and means `{nested}`; write the `&`, or add `scope(document)` to the definition to select a document-level element"
    )]
    ImplicitNestingSelector {
        /// Selector as written.
        selector: LeanStr,
        /// Selector with `&` inserted, which the variant applies.
        nested: LeanStr,
    },
    /// The header of a nested style rule or conditional at-rule applies the same CSS
    /// as `@variant <variant>` (`prefer-variant-directive`).
    #[error("{}", VariantDirectiveMessage { header, variant, px_to_rem: *px_to_rem })]
    PreferVariantDirective {
        /// Header text as written, such as `@media (width >= 40rem)`. Two nested
        /// headers are joined with ` { `.
        header: LeanStr,
        /// Variant spelling that the fix writes, such as `sm`.
        variant: LeanStr,
        /// Whether the match reads `px` lengths of a `@media` condition as `rem` at
        /// 16px = 1rem.
        px_to_rem: bool,
    },
    /// A later definition with the same name hides an earlier `@utility` or variant
    /// definition, so the earlier one is never used (`duplicate-definition`).
    #[error(
        "`{directive} {name}` is never used because a later definition with the same name replaces it"
    )]
    DuplicateDefinition {
        /// Directive of the hidden definition, such as `@utility` or `@custom-variant`.
        directive: LeanStr,
        /// Definition name as written.
        name: LeanStr,
    },
    /// A stylesheet definition outside every `@stillwind-registry` block registers
    /// after an undefinition of the `keyframe` or `view-transition` registry, so it
    /// reaches that registry although the undefinition removed the earlier definitions
    /// of its name (`bypassed-undefinition`).
    #[error(
        "`{directive} {name}` also defines `{name}` in the `{context}` registry, where `{undefinition} {pattern}` removed the earlier definitions"
    )]
    BypassedUndefinition {
        /// Directive of the definition, such as `@utility`.
        directive: LeanStr,
        /// Definition name as written.
        name: LeanStr,
        /// Registry that the definition reaches, such as `keyframe`.
        context: LeanStr,
        /// Undefinition directive, such as `@stillwind-undef-utility`.
        undefinition: LeanStr,
        /// Pattern of the undefinition as written.
        pattern: LeanStr,
    },
    /// A theme token value matches no value type of its namespace
    /// (`incompatible-theme-value`).
    #[error(
        "`{value}` is not a valid value for `{token}` because tokens of `--{namespace}-*` must be `{expected}`"
    )]
    IncompatibleThemeValue {
        /// Token name, including the leading `--`.
        token: LeanStr,
        /// Value as written.
        value: LeanStr,
        /// Namespace name without `--` and `-*`, such as `color`.
        namespace: LeanStr,
        /// Accepted type hints joined with ` or `.
        expected: LeanStr,
    },
    /// A stylesheet contains a Tailwind feature that Stillwind ignores
    /// (`ignored-tailwind-feature`).
    #[error("`{feature}` has no effect in Stillwind; {instead}")]
    IgnoredTailwindFeature {
        /// Feature as written, such as `@config` or `important`.
        feature: LeanStr,
        /// Clause that names the replacement or the resulting behavior.
        instead: LeanStr,
    },
    /// Loading skipped an invalid `@import` or `@plugin` statement, or an import that
    /// it cannot resolve or that closes a cycle (`invalid-css-import`).
    #[error("{reason}; the stylesheet is loaded without this import")]
    InvalidCssImport {
        /// Why loading skipped the import.
        reason: LeanStr,
    },
    /// Loading skipped an invalid Stillwind directive or definition
    /// (`invalid-css-directive`).
    #[error("{reason}")]
    InvalidCssDirective {
        /// Why loading skipped the statement.
        reason: LeanStr,
    },
    /// A definition is inside a conditional at-rule, and loading applies it without the
    /// condition (`ignored-definition-condition`).
    #[error("{reason}")]
    IgnoredDefinitionCondition {
        /// Why loading ignored the condition.
        reason: LeanStr,
    },
    /// A declaration has an `--if()` function that cannot be lowered, so the
    /// declaration is left out (`invalid-runtime-condition`).
    #[error("`{property}` is left out because its `--if()` cannot be lowered: {detail}")]
    InvalidRuntimeCondition {
        /// Property name of the declaration.
        property: LeanStr,
        /// Why the function cannot be lowered.
        detail: LeanStr,
    },
    /// A `theme()` or `--theme()` call names a theme token that is not active and has
    /// no fallback (`invalid-theme-reference`).
    #[error("`{reference}` names no theme token and has no fallback{}", DidYouMean(.suggestion))]
    UnresolvedThemeReference {
        /// Call text as written, such as `--theme(--color-nope)`.
        reference: LeanStr,
        /// Name of an active token with a close spelling.
        suggestion: Option<LeanStr>,
    },
    /// Every `--value()` domain of a `@utility` reads a theme namespace that provides
    /// no value, so no candidate resolves (`invalid-theme-reference`).
    #[error(
        "`@utility {utility}` reads values only from `{namespaces}`, but no theme token provides a value there, so no candidate can use it"
    )]
    EmptyThemeNamespace {
        /// Utility name as written, such as `box-*`.
        utility: LeanStr,
        /// Namespace patterns as written, such as `--size-*`, joined with `, `.
        namespaces: LeanStr,
    },
}

impl LintDiagnostic {
    /// Returns the rule selected by the diagnostic.
    #[allow(clippy::too_many_lines)]
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
            | Self::NestedMacroCall { .. }
            | Self::StyleArraySpread
            | Self::InvalidStyleInput
            | Self::StyleObjectSpread
            | Self::InvalidStyleTable
            | Self::InvalidStyleTableEntry
            | Self::InvalidStyleProperty
            | Self::InvalidComputedStyleKey
            | Self::InvalidStyleKey
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
            Self::InvalidMediaArgument { .. } | Self::NonMediaVariant { .. } => {
                LintKind::InvalidMediaQuery
            }
            Self::InvalidCssInput { .. } | Self::InvalidCssOption { .. } => {
                LintKind::InvalidCssInput
            }
            Self::InvalidVariantPrefix | Self::InvalidVariant { .. } => LintKind::InvalidVariant,
            Self::UnknownVariant { .. } => LintKind::UnknownVariant,
            Self::MissingDocumentTarget { .. } => LintKind::MissingDocumentTarget,
            Self::UnknownAnimationOption { .. } => LintKind::InvalidAnimationOption,
            Self::RuntimeConditionRequiresCssIf { .. } => LintKind::RuntimeConditionRequiresCssIf,
            Self::UnverifiedStyleBase => LintKind::UnverifiedStyleBase,
            Self::StyleBaseWithInlineStyle => LintKind::InvalidStyleComposition,
            Self::InvalidRuleGroupInput { .. } => LintKind::InvalidRuleGroupInput,
            Self::ArbitraryColor { .. } => LintKind::NoArbitraryColors,
            Self::ArbitrarySpacing { .. } => LintKind::NoArbitrarySpacing,
            Self::ClassProperty => LintKind::NoClassProperty,
            Self::RestrictedUtility { .. } => LintKind::NoRestrictedUtilities,
            Self::RestrictedDeclaration { .. } => LintKind::NoRestrictedDeclarations,
            Self::ImportantUtility | Self::ImportantDeclaration { .. } => LintKind::NoImportant,
            Self::InvalidLintSuppression { .. } => LintKind::InvalidLintSuppression,
            Self::UnusedLintSuppression { .. } => LintKind::UnusedLintSuppression,
            Self::IndirectStyling { .. } => LintKind::PreferDirectStyling,
            Self::DuplicateUtility { .. } => LintKind::DuplicateUtility,
            Self::ConflictingUtilities { .. } => LintKind::ConflictingUtilities,
            Self::PhysicalDeclaration { .. } => LintKind::PreferLogicalProperties,
            Self::RestrictedLengthUnit { .. } => LintKind::NoRestrictedLengthUnits,
            Self::RestrictedThemeToken { .. } => LintKind::NoRestrictedThemeTokens,
            Self::ArbitraryBreakpoint { .. } => LintKind::NoArbitraryBreakpoints,
            Self::NonCanonicalUtility { .. } => LintKind::PreferCanonicalUtilities,
            Self::UnsortedUtilities | Self::UnsortedStylesheet => LintKind::PreferSortedUtilities,
            Self::MismatchedFrameworkImport { .. } => LintKind::MismatchedFrameworkImport,
            Self::MutatingMacroArgument => LintKind::MutatingMacroArgument,
            Self::SuspendingMacroArgument => LintKind::SuspendingMacroArgument,
            Self::IneffectiveAnchorScope { .. } => LintKind::IneffectiveAnchorScope,
            Self::MissingRefAttachment { .. } => LintKind::MissingRefAttachment,
            Self::ProviderConflict { .. } => LintKind::ConflictingProviderProperties,
            Self::TailwindRelationshipUtility { .. } => LintKind::PreferTypedRelationships,
            Self::DocumentScopedSelector { .. } => LintKind::DocumentScopedSelector,
            Self::RepeatedNestingSelector { .. } => LintKind::RepeatedNestingSelector,
            Self::MissingViewTransitionName => LintKind::MissingViewTransitionName,
            Self::DuplicateViewTransitionName { .. } => LintKind::DuplicateViewTransitionName,
            Self::SplitViewTransitionClass => LintKind::SplitViewTransitionClass,
            Self::AmbiguousStyleBaseOrder { .. } => LintKind::AmbiguousStyleBaseOrder,
            Self::InterferingImportedStyleBase { .. } => LintKind::InterferingImportedStyleBase,
            Self::InvalidMarkerUse { .. } => LintKind::InvalidMarkerUse,
            Self::UnknownUtility { .. } => LintKind::UnknownUtility,
            Self::InvalidUtility { .. } => LintKind::InvalidUtility,
            Self::IncompatiblePropertyValue { .. }
            | Self::UnacceptedUtilityValue { .. }
            | Self::VariableTypeMismatch { .. }
            | Self::OpacityModifierOutOfRange { .. } => LintKind::IncompatiblePropertyValue,
            Self::RedundantVariant { .. } => LintKind::RedundantVariant,
            Self::UnsatisfiableVariants { .. } => LintKind::UnsatisfiableVariants,
            Self::ClampedValue { .. } => LintKind::ClampedValue,
            Self::UntypedVariable { .. } => LintKind::PreferTypedVariables,
            Self::ImplicitNestingSelector { .. } => LintKind::ImplicitNestingSelector,
            Self::PreferVariantDirective { .. } => LintKind::PreferVariantDirective,
            Self::DuplicateDefinition { .. } => LintKind::DuplicateDefinition,
            Self::InvalidCssImport { .. } => LintKind::InvalidCssImport,
            Self::InvalidRuntimeCondition { .. } => LintKind::InvalidRuntimeCondition,
            Self::InvalidCssDirective { .. } => LintKind::InvalidCssDirective,
            Self::IgnoredDefinitionCondition { .. } => LintKind::IgnoredDefinitionCondition,
            Self::BypassedUndefinition { .. } => LintKind::BypassedUndefinition,
            Self::IncompatibleThemeValue { .. } => LintKind::IncompatibleThemeValue,
            Self::IgnoredTailwindFeature { .. } => LintKind::IgnoredTailwindFeature,
            Self::UnresolvedThemeReference { .. } | Self::EmptyThemeNamespace { .. } => {
                LintKind::InvalidThemeReference
            }
        }
    }

    /// Returns the stable lint name used in reports and suppression directives.
    pub const fn code(&self) -> &'static str {
        self.kind().name()
    }
}
