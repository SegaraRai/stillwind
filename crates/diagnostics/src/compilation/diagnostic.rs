//! Fatal diagnostic payloads and message templates.

use serde::{Deserialize, Serialize};

use stillwind_source::{BytePos, CssSourceName, FileId, Position, Span, Utf16Offset};

use super::SourceFailureContext;

/// Typed causes and display templates for fatal compilation errors.
///
/// Each variant has a unique reporting code. The diagnostic owns cause data and its
/// message template; [`super::CompileError`] supplies the
/// reporting location, optional source name, and notes. Match variants to inspect causes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum CompileDiagnostic {
    /// The host-selected framework disagrees with the imported macro entry point.
    #[error("explicit framework conflicts with the imported style macro")]
    FrameworkConflict,
    /// Checker facts assign a symbolic value to an incompatible asset kind.
    #[error("invalid symbolic kinds in TypeScript facts")]
    InvalidSymbolicFacts,
    /// A style input array contains an omitted element.
    #[error("style() arrays cannot contain empty slots")]
    StyleArrayHole,
    /// A ref cannot be resolved locally without host checker facts.
    #[error("non-local or conditional refs require TypeScript checker facts")]
    RefFactsRequired,
    /// One module imports style macros for more than one framework.
    #[error("a module cannot mix framework style macros")]
    MixedFrameworks,
    /// A keyframes use appears where the call cannot become a class input.
    #[error("a keyframes use must be a class input of style()")]
    KeyframesUseNotAClassInput,
    /// A projection or ref variant was not consumed at a supported style input position.
    #[error(
        "{syntax} must be consumed at its supported input position within style(); it cannot be materialized independently"
    )]
    UnconsumedMacroUse {
        /// Display spelling of the rejected projection or variant.
        syntax: String,
    },
    /// A ref projection cannot be resolved locally without host checker facts.
    #[error("non-local or conditional ref projections require TypeScript checker facts")]
    RefProjectionFactsRequired,
    /// A container query variant has an invalid argument or interpolated query.
    #[error("ref.at() accepts no arguments, one literal query, or a static template")]
    InvalidContainerQuery,
    /// Style producers form a cycle within the composition graph.
    #[error("style composition dependency cycle")]
    CompositionCycle,
    /// Composition dependencies require a cycle in CSS module ordering.
    #[error("style composition requires cyclic CSS module ordering")]
    CompositionModuleCycle,
    /// A composition dependency has no available producer artifact.
    #[error("style base producer is unavailable")]
    UnavailableStyleBase,
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
    /// The JavaScript parser rejected the input; external details belong in wrapper notes.
    #[error("JavaScript parse error")]
    JavaScriptParse,
    /// Checker facts fail schema, revision, range, or payload validation.
    #[error("incompatible, stale or invalid TypeScript facts")]
    InvalidTypeScriptFacts,
    /// Generated CSS cannot be split into complete rule blocks.
    #[error("invalid generated CSS block at byte {offset}")]
    InvalidGeneratedCss {
        /// Zero-based UTF-8 byte offset within the offending generated CSS contribution.
        offset: usize,
    },
    /// An internal buffer was given a source-map entry for an original document.
    #[error("an internal source cannot have a source-map entry")]
    SourceInternalMap {
        /// Operation that attempted to register the buffer.
        context: SourceFailureContext,
    },
    /// An import-edit source was given a source-map entry without a URI.
    #[error("invalid macro import source: an internal source cannot have a source-map entry")]
    ImportSourceInternalMap,
    /// A source operation exceeds the supported 32-bit coordinate space.
    #[error("source size or offset {value} exceeds the 32-bit source limit")]
    SourceTooLarge {
        /// Requested source length or offset outside the supported coordinate space.
        value: usize,
        /// Operation that encountered this source failure.
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
        /// Operation that encountered this source failure.
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
        /// Operation that encountered this source failure.
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
        /// Operation that encountered this source failure.
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
        /// Operation that encountered this source failure.
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
    /// A converter produced code that the configured parser cannot read.
    #[error("cannot parse the code generated by the component converter: {detail}")]
    ProjectionSyntax {
        /// Parser explanation for the generated buffer.
        detail: String,
    },
    /// A converter or checker projection could not produce the required correspondence.
    #[error("cannot prepare the component projection: {detail}")]
    ProjectionPreparation {
        /// Explanation supplied by the projection adapter.
        detail: String,
    },
    /// A component parser rejected the original source.
    #[error("invalid component syntax: {detail}")]
    ComponentSyntax {
        /// Explanation supplied by the component parser.
        detail: String,
    },
    /// A required original expression has no usable checker target.
    #[error("cannot locate the checker expression: {detail}")]
    CheckerTargetUnavailable {
        /// Operation whose checker target is missing.
        detail: String,
    },
    /// An assignment changed the declaration identities carried by a value.
    #[error("incompatible declaration identities: {detail}")]
    IdentityAssignment {
        /// Checker explanation for the assignment.
        detail: String,
    },
    /// A value no longer identifies a finite set of Stillwind declarations.
    #[error("cannot determine finite declaration identities: {detail}")]
    IdentityUnavailable {
        /// Checker explanation for the value.
        detail: String,
    },
    /// A generated span has no unique corresponding source range.
    #[error("cannot verify the original expression or declaration after component conversion")]
    ProjectionUnmapped {
        /// Generated-source range for which no unique original range exists.
        span: Span,
    },
    /// Mapped generated text differs from its original source text.
    #[error("generated expression or declaration does not match the original source")]
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
    #[error("cannot verify every reference to macro `{local}` after component conversion")]
    UnmappedMacroReference {
        /// Local name of the macro binding.
        local: String,
        /// Macro binding range whose references lack verified source correspondence.
        span: Span,
    },
    /// An authored theme lookup has neither a resolved token nor a usable fallback.
    #[error("authored CSS contains an unresolved --theme() reference without a usable fallback")]
    UnresolvedThemeReference,
    /// A `--theme()` call has invalid syntax at a byte offset within its value.
    #[error("invalid --theme() reference at value byte {offset}: {detail}")]
    InvalidThemeReference {
        /// Zero-based UTF-8 byte offset within the containing value.
        offset: usize,
        /// Syntax error reported by the theme resolver.
        detail: String,
    },
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
    /// A requirement directive contains malformed names or syntax.
    #[error("invalid Stillwind property requirement at byte {offset}")]
    MalformedPropertyRequirement {
        /// Byte offset of the directive within its CSS body.
        offset: usize,
    },
    /// A matched utility definition contains a malformed requirement directive.
    #[error("utility `{utility}` has an invalid property requirement at body byte {offset}")]
    MalformedUtilityRequirement {
        /// Registered utility name.
        utility: String,
        /// Byte offset within the definition body.
        offset: usize,
    },
    /// A generated source transform contains invalid expression mapping metadata.
    #[error("invalid expression mapping in source replacement at {span:?}")]
    InvalidTransformExpressionMapping {
        /// Original call span carrying the replacement.
        span: Span,
    },
    /// Generated source replacements overlap.
    #[error("overlapping source replacement {span:?}, previous start {previous_start}")]
    InvalidTransformOverlap {
        /// Later replacement that overlaps a preceding one.
        span: Span,
        /// Start offset of the preceding replacement.
        previous_start: u32,
    },
    /// Rendered source exceeds the supported coordinate range.
    #[error("rendered source exceeds the 32-bit coordinate limit")]
    InvalidTransformSize,
    /// A failed compilation was passed to source-transform rendering.
    #[error("source replacement was requested for a failed compilation")]
    InvalidTransformState,
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
    /// One host source name was supplied with differing document identity or contents.
    #[error("conflicting CSS source for {name}")]
    ConflictingCssSource {
        /// Opaque source name supplied by the host.
        name: CssSourceName,
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
        /// Rejected prefix supplied by the host.
        value: String,
    },
    /// A configured generated-name hash length cannot spell one request digest.
    #[error("hashLength must be between 1 and {maximum} base-36 digits, not {length}")]
    InvalidHashLength {
        /// Rejected length supplied by the host or configuration.
        length: usize,
        /// Longest base-36 spelling of one request digest.
        maximum: usize,
    },
    /// The generated-property namespaces use the same prefix.
    #[error("prefix and tailwindPrefix must be different")]
    ConflictingPropertyPrefixes {
        /// Prefix shared by the two namespaces.
        prefix: String,
    },
    /// A lint policy cannot be evaluated with the supplied options.
    #[error("invalid `{rule}` lint configuration: {detail}")]
    InvalidLintConfiguration {
        /// Stable lint rule name.
        rule: String,
        /// Invalid field or missing required policy.
        detail: String,
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
