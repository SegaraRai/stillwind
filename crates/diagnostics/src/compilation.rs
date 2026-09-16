//! Fatal causes, reporting codes, and source-located compilation errors.
//!
//! Producers construct a [`CompileErrorKind`] and attach a valid reporting span with
//! [`CompileError::new`]. Source/projection conversions preserve rejected coordinates
//! as cause data; they do not choose a valid primary label or perform detection.

use serde::{Deserialize, Serialize};

use stillwind_source::{
    BytePos, CoordinateError, FileId, Position, ProjectionError, SourceError, Span, Utf16Offset,
};

use crate::Label;

/// Typed causes and display templates for fatal compilation errors.
///
/// Codes are reporting identities, not discriminants: distinct causes may share a code.
/// The kind owns cause data and its message template; [`CompileError`] supplies the
/// reporting location, optional source name, and notes. Match variants to inspect causes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum CompileErrorKind {
    /// The host-selected framework disagrees with the imported macro entry point.
    #[error("explicit framework conflicts with the imported style macro")]
    FrameworkConflict,
    /// Checker facts assign a symbolic value to an incompatible asset kind.
    #[error("invalid symbolic kinds in TypeScript facts")]
    InvalidSymbolicFacts,
    /// A composition base is an inline macro call instead of a bound style value.
    #[error("nested style() calls are not supported; bind the base to a variable")]
    NestedStyleCall,
    /// A style input array contains a spread that cannot be statically expanded.
    #[error("style() array spreads require a literal array")]
    StyleArraySpread,
    /// A style input array contains an omitted element.
    #[error("style() arrays cannot contain empty slots")]
    StyleArrayHole,
    /// A ref cannot be resolved locally without host checker facts.
    #[error("non-local or conditional refs require TypeScript checker facts")]
    RefFactsRequired,
    /// An expression is outside the supported static style-input forms.
    #[error(
        "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and variant wrappers"
    )]
    InvalidStyleInput,
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
        "style() object keys must be `class`, quoted aria-*/data-* attributes, --* custom properties, or variant prefixes ending in `:`"
    )]
    InvalidAttributeKey,
    /// A variant wrapper contains an unsupported input expression.
    #[error(
        "variant wrappers accept utility strings, keyframes, arrays, and nested variant wrappers"
    )]
    InvalidVariantInput,
    /// An interpolation cannot be lowered as a complete ref-qualified utility segment.
    #[error("utility templates require direct ref variants in complete `variant:utility` segments")]
    InvalidUtilityTemplate,
    /// One module imports style macros for more than one framework.
    #[error("a module cannot mix framework style macros")]
    MixedFrameworks,
    /// A style call contains a spread argument whose elements cannot be resolved.
    #[error("style() argument spreads require a literal array")]
    StyleArgumentSpread,
    /// A keyframes use does not supply one literal options string.
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
    /// No valid frame remains after validating a keyframes declaration.
    #[error("style.keyframes() requires at least one valid keyframe")]
    EmptyKeyframes,
    /// A keyed transition use supplies more than one key expression.
    #[error("viewTransition.for() accepts at most one key expression")]
    InvalidViewTransitionKeyExpression,
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
    /// No valid phase remains after validating a transition declaration.
    #[error("style.viewTransition() requires at least one of group, old, new, or imagePair")]
    EmptyViewTransition,
    /// A single ref argument is neither a literal name nor an expression that resolves
    /// to exactly one ref identity.
    #[error(
        "style.ref() accepts an optional literal name or exactly one statically resolved ref identity"
    )]
    InvalidRefArgument,
    /// The first argument of a two-argument ref declaration does not resolve to exactly
    /// one ref identity.
    #[error(
        "the boundary argument of style.ref() must resolve statically to exactly one ref identity"
    )]
    InvalidRefBoundary,
    /// The second ref argument is not a literal name.
    #[error("the second style.ref() argument must be a literal name")]
    InvalidRefNameArgument,
    /// A ref declaration uses an unsupported argument count.
    #[error("style.ref() accepts (), (name), (boundary), or (boundary, name)")]
    InvalidRefArguments,
    /// A ref variant's selector cannot be determined from literal text.
    #[error("ref variants require a static variant string")]
    NonStaticRefVariant,
    /// A variant receiver lacks the ref identity needed for selector generation.
    #[error("ref variants require a statically identified style.ref()")]
    UnidentifiedRefVariant,
    /// A container projection does not supply a literal container type.
    #[error("ref.container() requires a literal container type")]
    InvalidContainerType,
    /// A ref projection cannot be resolved locally without host checker facts.
    #[error("non-local or conditional ref projections require TypeScript checker facts")]
    RefProjectionFactsRequired,
    /// A container query projection lacks its literal query and utility arguments.
    #[error("ref.at() requires one nonempty literal query or a static template")]
    InvalidContainerQuery,
    /// A projection receiver is not an identity created by a ref declaration.
    #[error("ref projections must use an identity returned by style.ref()")]
    InvalidRefProjection,
    /// Style producers form a cycle within the composition graph.
    #[error("style composition dependency cycle")]
    CompositionCycle,
    /// Composition dependencies require a cycle in CSS module ordering.
    #[error("style composition requires cyclic CSS module ordering")]
    CompositionModuleCycle,
    /// A composition dependency has no available producer artifact.
    #[error("style base producer is unavailable")]
    UnavailableStyleBase,
    /// A composition base carries props that cannot be merged as class output.
    #[error("style bases with additional props are not supported")]
    StyleBaseWithProps,
    /// A composition base uses a different framework class property.
    #[error("style bases must use the same framework class property")]
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
    /// A variant prefix is syntactically malformed.
    #[error("invalid variant prefix")]
    InvalidVariantPrefix,
    /// A syntactically parsed variant prefix cannot be resolved in the theme.
    #[error("unknown or unavailable variant prefix")]
    UnavailableVariantPrefix,
    /// A ref wrapper has no known identity and cannot safely emit a selector.
    #[error("ref variant has no known ref identity")]
    UnidentifiedRef,
    /// Two generated symbols would publish the same identifier.
    #[error("generated identifier collision: {name}")]
    GeneratedIdentifierCollision {
        /// Identifier whose generated definitions collide.
        name: Box<str>,
    },
    /// A host-provided identifier generator failed for one name.
    #[error("identifier generation failed for {0}: {1}")]
    IdentifierGenerationFailed(
        /// Identifier family the generator was asked for.
        Box<str>,
        /// Generator-supplied failure description, not interpreted by the compiler.
        Box<str>,
    ),
    /// A utility required by fatal compilation cannot be resolved.
    #[error("unknown or unavailable utility `{utility}`")]
    UnavailableUtility {
        /// Utility text as authored, including variants.
        utility: String,
    },
    /// An animation option token is not recognized by the option resolver.
    #[error("unknown animation option `{option}`")]
    UnknownAnimationOption {
        /// Name of the invalid configuration option.
        option: String,
    },
    /// The JavaScript parser rejected the input; external details belong in wrapper notes.
    #[error("JavaScript parse error")]
    JavaScriptParse,
    /// Checker facts were supplied through an entry point that cannot map projected facts.
    #[error("checker facts for projected sources are not supported")]
    ProjectedFactsUnsupported,
    /// Checker facts fail schema, revision, range, or payload validation.
    #[error("incompatible, stale or invalid TypeScript facts")]
    InvalidTypeScriptFacts,
    /// Generated CSS cannot be split into complete rule blocks.
    #[error("invalid generated CSS block at byte {offset}")]
    InvalidGeneratedCss {
        /// Zero-based UTF-8 byte offset within the offending generated CSS contribution.
        offset: usize,
    },
    /// A source operation exceeds the supported 32-bit coordinate space.
    #[error("source size or offset {value} exceeds the 32-bit source limit")]
    SourceTooLarge {
        /// Requested source length or offset outside the supported coordinate space.
        value: usize,
        /// Operation reporting this source failure; determines the error code.
        context: SourceFailureContext,
    },
    /// Import-edit validation exceeds the supported source-coordinate space.
    #[error(
        "invalid macro import source span: source size or offset {value} exceeds the 32-bit source limit"
    )]
    ImportSourceTooLarge {
        /// Requested source length or offset outside the supported coordinate space.
        value: usize,
    },
    /// A source operation requested an unusable byte range.
    #[error("invalid source byte range {start}..{end}")]
    SourceInvalidRange {
        /// Inclusive byte offset of the rejected range.
        start: usize,
        /// Exclusive byte offset of the rejected range.
        end: usize,
        /// Operation reporting this source failure; determines the error code.
        context: SourceFailureContext,
    },
    /// Import-edit validation requested an unusable byte range.
    #[error("invalid macro import source span: invalid source byte range {start}..{end}")]
    ImportSourceInvalidRange {
        /// Inclusive byte offset of the rejected range.
        start: usize,
        /// Exclusive byte offset of the rejected range.
        end: usize,
    },
    /// A span was applied to a different source file.
    #[error("span belongs to file {actual:?}, expected {expected:?}")]
    SourceWrongFile {
        /// File identity required by the source buffer.
        expected: FileId,
        /// File identity carried by the rejected span.
        actual: FileId,
        /// Operation reporting this source failure; determines the error code.
        context: SourceFailureContext,
    },
    /// Import-edit metadata points into a different source file.
    #[error(
        "invalid macro import source span: span belongs to file {actual:?}, expected {expected:?}"
    )]
    ImportSourceWrongFile {
        /// File identity required by the source buffer.
        expected: FileId,
        /// File identity carried by the rejected span.
        actual: FileId,
    },
    /// A source operation rejected a span's bounds or UTF-8 boundaries.
    #[error("invalid source span {span:?}")]
    SourceInvalidSpan {
        /// Rejected span, including its source identity and half-open UTF-8 byte range.
        span: Span,
        /// Operation reporting this source failure; determines the error code.
        context: SourceFailureContext,
    },
    /// Import-edit validation rejected a span's bounds or UTF-8 boundaries.
    #[error("invalid macro import source span: invalid source span {span:?}")]
    ImportSourceInvalidSpan {
        /// Rejected span, including its source identity and half-open UTF-8 byte range.
        span: Span,
    },
    /// A source operation rejected a UTF-8 byte position.
    #[error("invalid byte offset {offset:?}")]
    SourceInvalidOffset {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: BytePos,
        /// Operation reporting this source failure; determines the error code.
        context: SourceFailureContext,
    },
    /// Import-edit validation rejected a UTF-8 byte position.
    #[error("invalid macro import source span: invalid byte offset {offset:?}")]
    ImportSourceInvalidOffset {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: BytePos,
    },
    /// A framework projection map failed structural validation.
    #[error("invalid projection map: {detail}")]
    ProjectionMap {
        /// Structural validation explanation from the projection map.
        detail: String,
    },
    /// A generated span has no unique corresponding source range.
    #[error("projection has no unique original location for {span:?}")]
    ProjectionUnmapped {
        /// Generated-source range for which no unique original range exists.
        span: Span,
    },
    /// Mapped generated text differs from its original source text.
    #[error("projected text does not match original source at {span:?}")]
    ProjectionContent {
        /// Range involved in the failed generated-to-original text correspondence check.
        span: Span,
    },
    /// A UTF-16 code-unit offset cannot be converted to a source position.
    #[error("invalid UTF-16 offset {offset:?}")]
    InvalidUtf16Offset {
        /// Rejected zero-based UTF-16 code-unit offset.
        offset: Utf16Offset,
    },
    /// A line and UTF-16 column cannot be converted to a source offset.
    #[error("invalid UTF-16 position {position:?}")]
    InvalidUtf16Position {
        /// Rejected zero-based line and UTF-16 column.
        position: Position,
    },
    /// A UTF-8 byte offset cannot be converted to a source position.
    #[error("invalid UTF-8 byte offset {offset:?}")]
    InvalidUtf8Offset {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: BytePos,
    },
    /// A runtime use has no verified macro import in its import realm.
    #[error("no verified macro import anchor for runtime use at {span:?}")]
    MissingRuntimeAnchor {
        /// Runtime-use range whose import realm lacks a verified macro anchor.
        span: Span,
    },
    /// Import spans or realm metadata are inconsistent with the source.
    #[error("invalid macro import metadata at {span:?}")]
    InvalidImportMetadata {
        /// Import declaration or runtime-use range associated with inconsistent metadata.
        span: Span,
    },
    /// An explicit CSS import lacks the host-provided virtual module target.
    #[error("explicit module CSS import requires a host-allocated virtual module at {span:?}")]
    MissingModuleCssTarget {
        /// Explicit module-CSS import range that requires a host-provided target.
        span: Span,
    },
    /// A macro read remains after lowering, preventing removal of its binding.
    #[error("macro binding `{local}` has a reference that was not transformed at {span:?}")]
    UntransformedMacroReference {
        /// Local name of the macro binding.
        local: String,
        /// Range of the macro read left intact by lowering.
        span: Span,
    },
    /// A macro binding has reads without verified projection correspondence.
    #[error(
        "macro binding `{local}` has references without verified source correspondence at {span:?}"
    )]
    UnmappedMacroReference {
        /// Local name of the macro binding.
        local: String,
        /// Macro binding range whose references lack verified source correspondence.
        span: Span,
    },
    /// An authored theme lookup has neither a resolved token nor a usable fallback.
    #[error("authored CSS contains an unresolved --theme() reference without a usable fallback")]
    UnresolvedThemeReference,
    /// The output template has no destination for generated utility rules.
    #[error("generated utilities require an @stillwind-slot utilities; in the CSS entry graph")]
    MissingUtilitiesSlot,
    /// The output template lacks a destination for generated support CSS.
    #[error("generated support requires an @stillwind-slot {slot}; in the CSS entry graph")]
    MissingPropertySlot {
        /// Slot name without the `@stillwind-slot` directive or trailing semicolon.
        slot: String,
    },
    /// An authored registration requirement cannot be parsed.
    #[error("invalid Stillwind property requirement")]
    InvalidPropertyRequirement,
    /// An experimental runtime condition cannot be lowered using the selected output strategy.
    #[error("invalid runtime conditional: {detail}")]
    InvalidRuntimeConditional {
        /// Syntax or output-strategy limitation preventing compilation.
        detail: String,
    },
    /// CSS parsing failed while collecting custom-property usage.
    #[error("CSS parse error: {detail}")]
    PropertyUsage {
        /// Display text from the CSS parser used to collect custom-property usage.
        detail: String,
    },
    /// A stylesheet contribution refers to a rule absent from the retained artifact.
    #[error("stylesheet contribution references a missing rule")]
    MissingRule,
    /// The loader's source graph is inconsistent with its tracked dependencies.
    #[error("inconsistent CSS source graph")]
    InvalidCssGraph,
    /// The loader cannot allocate another request identifier.
    #[error("CSS request IDs exhausted")]
    CssRequestIdExhausted,
    /// The loader cannot allocate another source-file identifier.
    #[error("too many CSS sources")]
    CssSourceIdExhausted,
    /// A file response was supplied after the loader reached a terminal state.
    #[error("provide called after completion")]
    CssLoadAlreadyComplete,
    /// The first loader response does not contain exactly one root source.
    #[error("initial provide must contain exactly one file (conventionally ID 0), got {count}")]
    InvalidCssInitialBatch {
        /// Number of supplied root sources.
        count: usize,
    },
    /// A response does not correspond to one outstanding loader request.
    #[error("unknown or duplicate request ID {id}")]
    UnknownCssRequest {
        /// Request identifier that was unknown or already answered.
        id: u32,
    },
    /// One host source identity was supplied with different contents.
    #[error("conflicting content for {name}")]
    ConflictingCssContent {
        /// Opaque source name supplied by the host.
        name: String,
    },
    /// Resolved imports contain a dependency cycle.
    #[error("CSS import cycle: {requests}")]
    CssImportCycle {
        /// Debug representation of the ordered host file requests.
        requests: String,
    },
    /// Single-buffer loading encountered imports requiring host resolution.
    #[error("host must resolve CSS imports using DesignSystemLoader: {requests}")]
    UnresolvedCssImports {
        /// Debug representation of the ordered host file requests.
        requests: String,
    },
    /// A configured custom-property prefix fails the required shape or minimum length.
    #[error("{option} must start with `--` and end with `-`")]
    InvalidPropertyPrefix {
        /// Configuration field name: `prefix` or `tailwindPrefix`.
        option: String,
        /// Rejected prefix supplied by the host or CSS configuration.
        value: String,
    },
    /// The generated-property namespaces use the same prefix.
    #[error("prefix and tailwindPrefix must be different")]
    ConflictingPropertyPrefixes {
        /// Prefix shared by the two namespaces.
        prefix: String,
    },
    /// A compile-time definition appears beneath a conditional CSS wrapper.
    #[error(
        "compile-time @{directive} definitions cannot be conditional (byte {offset}); import definitions unconditionally and put conditions around @stillwind-slot utilities;"
    )]
    ConditionalDefinition {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// At-rule name without the leading `@`.
        directive: String,
    },
    /// An authored output directive names an unsupported slot.
    #[error("unsupported Stillwind output slot at byte {offset}: {slot}")]
    UnsupportedOutputSlot {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Slot name without the `@stillwind-slot` directive or trailing semicolon.
        slot: String,
    },
    /// An output slot appears in a CSS context that cannot contain its output.
    #[error("Stillwind output slot {slot} has an invalid CSS context")]
    InvalidOutputSlotContext {
        /// Slot name without the `@stillwind-slot` directive or trailing semicolon.
        slot: String,
    },
    /// An authored output block cannot be parsed as a complete block.
    #[error("invalid CSS output block at byte {offset}")]
    InvalidCssOutputBlock {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
    },
    /// A parsed CSS position cannot be represented by source coordinates.
    #[error("CSS source offset {offset} exceeds the 32-bit source limit")]
    CssOffsetTooLarge {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
    },
    /// The retained authored CSS failed stylesheet parsing.
    #[error("CSS parse error: {detail}")]
    CssParse {
        /// Display text from the authored stylesheet parser.
        detail: String,
    },
    /// A theme block has no closing delimiter.
    #[error("unterminated @theme block")]
    UnterminatedTheme {},
    /// A CSS block has no closing delimiter.
    #[error("unterminated CSS block at byte {offset}")]
    UnterminatedCssBlock {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
    },
    /// A CSS construct nests deeper than the depth the scanner can process.
    #[error("CSS nesting exceeds {limit} levels at byte {offset}")]
    CssNestingTooDeep {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Deepest nesting level the failing operation accepts.
        limit: usize,
    },
    /// An import modifier uses an unsupported form.
    #[error("unsupported import modifier at byte {offset}: {modifier}")]
    UnsupportedImportModifier {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Unsupported CSS import modifier.
        modifier: String,
    },
    /// Import modifiers occur outside the supported layer/supports/media order.
    #[error("invalid import modifier order at byte {offset}")]
    InvalidImportModifierOrder {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
    },
    /// A block encountered while scanning imports cannot be consumed.
    #[error("invalid CSS block at byte {offset}: {detail}")]
    InvalidImportBlock {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Explanation supplied by the underlying parser or map validator.
        detail: String,
    },
    /// An import does not begin with a valid string or URL specifier.
    #[error("invalid @import at byte {offset}: {detail}")]
    InvalidImportSpecifier {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Explanation supplied by the underlying parser or map validator.
        detail: String,
    },
    /// The trailing import modifier list cannot be parsed.
    #[error("invalid import modifiers at byte {offset}: {detail}")]
    InvalidImportModifiers {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Explanation supplied by the underlying parser or map validator.
        detail: String,
    },
    /// A functional import modifier has an invalid argument block.
    #[error("invalid import modifier at byte {offset}: {detail}")]
    InvalidImportModifierBlock {
        /// Zero-based UTF-8 byte offset in the buffer parsed by the failing operation.
        offset: usize,
        /// Explanation supplied by the underlying parser or map validator.
        detail: String,
    },
}

impl CompileErrorKind {
    /// Returns the stable fatal-error reporting code, shared by related causes.
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete cause-to-code mapping exhaustive in one place."
    )]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::FrameworkConflict | Self::MixedFrameworks => "SHJS008",
            Self::InvalidSymbolicFacts
            | Self::ProjectedFactsUnsupported
            | Self::InvalidTypeScriptFacts => "SHFACT001",
            Self::NestedStyleCall => "SHCOMP001",
            Self::StyleArraySpread
            | Self::StyleArrayHole
            | Self::InvalidStyleInput
            | Self::StyleArgumentSpread => "SHJS002",
            Self::RefFactsRequired
            | Self::UnidentifiedRefVariant
            | Self::RefProjectionFactsRequired => "SHTS001",
            Self::AttributeSpread
            | Self::InvalidAttributeProperty
            | Self::InvalidComputedAttribute
            | Self::InvalidAttributeKey => "SHJS003",
            Self::InvalidVariantInput => "SHJS004",
            Self::InvalidUtilityTemplate => "SHJS005",
            Self::InvalidKeyframesOptions | Self::UnknownAnimationOption { .. } => "SHANIMATION002",
            Self::InvalidKeyframesDefinition
            | Self::KeyframesSpread
            | Self::InvalidKeyframesProperty
            | Self::InvalidKeyframesFrameKey
            | Self::InvalidKeyframesFrameSelector
            | Self::InvalidKeyframesFrameValue
            | Self::EmptyKeyframes => "SHANIMATION001",
            Self::InvalidViewTransitionKeyExpression => "SHVIEW002",
            Self::InvalidViewTransitionDefinition
            | Self::ViewTransitionSpread
            | Self::InvalidViewTransitionProperty
            | Self::InvalidViewTransitionKey
            | Self::InvalidViewTransitionValue
            | Self::EmptyViewTransition => "SHVIEW001",
            Self::InvalidRefArgument
            | Self::InvalidRefBoundary
            | Self::InvalidRefNameArgument
            | Self::InvalidRefArguments
            | Self::InvalidRefProjection => "SHREF001",
            Self::NonStaticRefVariant => "SHREF005",
            Self::InvalidContainerType => "SHREF002",
            Self::InvalidContainerQuery => "SHREF004",
            Self::CompositionCycle => "SHCOMP003",
            Self::CompositionModuleCycle => "SHCOMP004",
            Self::UnavailableStyleBase => "SHCOMP005",
            Self::StyleBaseWithProps => "SHCOMP006",
            Self::StyleBaseFrameworkMismatch => "SHCOMP007",
            Self::KeyframesFrameVariants => "SHANIMATION003",
            Self::InvalidKeyframesVariant => "SHANIMATION004",
            Self::InvalidViewTransitionVariant => "SHVIEW003",
            Self::InvalidVariantPrefix | Self::UnavailableVariantPrefix | Self::UnidentifiedRef => {
                "SHREF003"
            }
            Self::GeneratedIdentifierCollision { .. } => "SHSYMBOL001",
            Self::IdentifierGenerationFailed { .. } => "SHSYMBOL002",
            Self::UnavailableUtility { .. } => "SH001",
            Self::JavaScriptParse => "SHJS001",
            Self::InvalidGeneratedCss { .. }
            | Self::UnresolvedThemeReference
            | Self::MissingUtilitiesSlot
            | Self::MissingPropertySlot { .. }
            | Self::InvalidPropertyRequirement
            | Self::InvalidRuntimeConditional { .. }
            | Self::PropertyUsage { .. }
            | Self::MissingRule
            | Self::InvalidCssGraph
            | Self::CssRequestIdExhausted
            | Self::CssSourceIdExhausted
            | Self::CssLoadAlreadyComplete
            | Self::InvalidCssInitialBatch { .. }
            | Self::UnknownCssRequest { .. }
            | Self::ConflictingCssContent { .. }
            | Self::CssImportCycle { .. }
            | Self::UnresolvedCssImports { .. }
            | Self::InvalidPropertyPrefix { .. }
            | Self::ConflictingPropertyPrefixes { .. }
            | Self::ConditionalDefinition { .. }
            | Self::UnsupportedOutputSlot { .. }
            | Self::InvalidOutputSlotContext { .. }
            | Self::InvalidCssOutputBlock { .. }
            | Self::CssOffsetTooLarge { .. }
            | Self::CssParse { .. }
            | Self::UnterminatedTheme { .. }
            | Self::UnterminatedCssBlock { .. }
            | Self::CssNestingTooDeep { .. }
            | Self::UnsupportedImportModifier { .. }
            | Self::InvalidImportModifierOrder { .. }
            | Self::InvalidImportBlock { .. }
            | Self::InvalidImportSpecifier { .. }
            | Self::InvalidImportModifiers { .. }
            | Self::InvalidImportModifierBlock { .. } => "SHCSS001",
            Self::SourceTooLarge { context, .. }
            | Self::SourceInvalidRange { context, .. }
            | Self::SourceWrongFile { context, .. }
            | Self::SourceInvalidSpan { context, .. }
            | Self::SourceInvalidOffset { context, .. } => context.code(),
            Self::ImportSourceTooLarge { .. }
            | Self::ImportSourceInvalidRange { .. }
            | Self::ImportSourceWrongFile { .. }
            | Self::ImportSourceInvalidSpan { .. }
            | Self::ImportSourceInvalidOffset { .. }
            | Self::MissingRuntimeAnchor { .. }
            | Self::InvalidImportMetadata { .. }
            | Self::MissingModuleCssTarget { .. }
            | Self::UntransformedMacroReference { .. }
            | Self::UnmappedMacroReference { .. } => "SHIMPORT001",
            Self::ProjectionMap { .. }
            | Self::ProjectionUnmapped { .. }
            | Self::ProjectionContent { .. }
            | Self::InvalidUtf16Offset { .. }
            | Self::InvalidUtf16Position { .. }
            | Self::InvalidUtf8Offset { .. } => "SHPRJ001",
        }
    }
}

impl CompileErrorKind {
    /// Preserves source-failure fields in an import-edit cause with code `SHIMPORT001`.
    /// The caller attaches a valid reporting span separately; rejected coordinates may be invalid.
    pub fn import_source_failure(error: &SourceError) -> Self {
        match *error {
            SourceError::TooLarge(value) => Self::ImportSourceTooLarge { value },
            SourceError::InvalidRange { start, end } => {
                Self::ImportSourceInvalidRange { start, end }
            }
            SourceError::WrongFile { expected, actual } => {
                Self::ImportSourceWrongFile { expected, actual }
            }
            SourceError::InvalidSpan(span) => Self::ImportSourceInvalidSpan { span },
            SourceError::InvalidOffset(offset) => Self::ImportSourceInvalidOffset { offset },
        }
    }

    /// Preserves source-failure fields and selects a reporting code using `context`.
    /// This does not validate coordinates or attach a primary reporting location.
    pub fn source_failure(error: &SourceError, context: SourceFailureContext) -> Self {
        match *error {
            SourceError::TooLarge(value) => Self::SourceTooLarge { value, context },
            SourceError::InvalidRange { start, end } => Self::SourceInvalidRange {
                start,
                end,
                context,
            },
            SourceError::WrongFile { expected, actual } => Self::SourceWrongFile {
                expected,
                actual,
                context,
            },
            SourceError::InvalidSpan(span) => Self::SourceInvalidSpan { span, context },
            SourceError::InvalidOffset(offset) => Self::SourceInvalidOffset { offset, context },
        }
    }
}

impl From<ProjectionError> for CompileErrorKind {
    fn from(error: ProjectionError) -> Self {
        match error {
            ProjectionError::Map(detail) => Self::ProjectionMap { detail },
            ProjectionError::Source(error) => {
                Self::source_failure(&error, SourceFailureContext::Projection)
            }
            ProjectionError::Coordinate(error) => match error {
                CoordinateError::Offset(offset) => Self::InvalidUtf16Offset { offset },
                CoordinateError::Position(position) => Self::InvalidUtf16Position { position },
                CoordinateError::Byte(offset) => Self::InvalidUtf8Offset { offset },
            },
            ProjectionError::Unmapped(span) => Self::ProjectionUnmapped { span },
            ProjectionError::Content(span) => Self::ProjectionContent { span },
        }
    }
}

/// One fatal error with its source highlights and supplementary context.
///
/// [`Self::new`] attaches one primary label; callers may add highlights and notes.
/// Display includes the optional source name and cause, while labels and notes are
/// formatted separately by the host. Fatal errors do not participate in lint suppression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{}{kind}", .source_name.as_ref().map_or_else(String::new, |name| format!("{name}: ")))]
pub struct CompileError {
    /// Optional opaque source name, displayed verbatim followed by `: ` before the cause.
    pub source_name: Option<String>,
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub kind: CompileErrorKind,
    /// Ordered source highlights; the first is the primary reporting location.
    pub labels: Vec<Label>,
    /// Ordered supplementary explanations, including external parser details; not part of Display.
    pub notes: Vec<String>,
}

impl CompileError {
    /// Attaches `span` as the sole primary label, with no source name or notes.
    /// The caller supplies a valid reporting span; rejected coordinates remain in `kind`.
    pub fn new(kind: CompileErrorKind, span: Span) -> Self {
        Self {
            source_name: None,
            kind,
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }

    /// Returns the error with `name` replacing any existing display source name.
    /// Labels, notes, and the cause are retained; the name is displayed verbatim as a prefix.
    #[must_use]
    pub fn with_source_name(mut self, name: impl Into<String>) -> Self {
        self.source_name = Some(name.into());
        self
    }

    /// Returns the cause's stable reporting code.
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }
}

/// Operation that reports a source-coordinate failure with its own stable code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceFailureContext {
    /// Utility tokenization (`SHSPAN001`).
    Utility,
    /// Framework projection (`SHPRJ001`).
    Projection,
    /// CSS source loading (`SHCSS001`).
    Stylesheet,
}

impl SourceFailureContext {
    /// Returns the reporting code for the operation.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Utility => "SHSPAN001",
            Self::Projection => "SHPRJ001",
            Self::Stylesheet => "SHCSS001",
        }
    }
}
