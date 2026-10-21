//! Named ref, animation, and view-transition declarations and use identities.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, Span, Spanned};

use super::StyleVariant;

/// A `style.ref()` declaration serving as an identity and a prop producer.
///
/// A marker attachment and a named anchor or container share this declaration identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefDefinition {
    /// Module identity of the declaring module.
    pub module_id: ModuleId,
    /// Lexical binding of the ref; its span seeds the declaration identity.
    pub binding: Spanned<LeanStr>,
    /// Optional user label that participates in the declaration's generated names.
    pub label: Option<Spanned<LeanStr>>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
}

/// A `style.keyframes()` declaration: named keyframes plus their identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframesDefinition {
    /// Local binding; its span seeds the declaration identity.
    pub binding: Spanned<LeanStr>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
    /// Authored label, which the built-in generator spells into the keyframes name.
    #[serde(default)]
    pub label: Option<Spanned<LeanStr>>,
    /// Default frames and complete conditional replacements.
    pub body: KeyframesBody,
}

/// A complete keyframes definition with optional conditional replacements.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframesBody {
    /// Frame declarations in authored order; omitted frames are not inherited.
    pub frames: Vec<KeyframesFrame>,
    /// Nested conditions conjoin with their ancestors and replace the complete
    /// definition.
    pub branches: Vec<KeyframesBranch>,
}

/// A condition selecting a complete keyframes definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframesBranch {
    /// Variant conditions resolved at the keyframes rule boundary.
    pub variant: StyleVariant,
    /// Complete replacement frames and further conditional replacements.
    pub body: KeyframesBody,
}

/// One `"from"` / `"to"` / percentage frame of a named keyframes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframesFrame {
    /// Frame selector text: `from`, `to`, or a percentage list.
    pub selector: Spanned<LeanStr>,
    /// Utility list applied inside the frame, in source order.
    pub utilities: Spanned<LeanStr>,
}

/// A `style.viewTransition()` declaration. Uses of one declaration share a single
/// generated class; the optional key stays a user expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewTransitionDefinition {
    /// Local binding; its span seeds the declaration identity. Every use shares it.
    pub binding: Spanned<LeanStr>,
    /// Span of the producing call, used for replacement.
    pub call_span: Span,
    /// Authored label, which the built-in generator spells into the class token.
    #[serde(default)]
    pub label: Option<Spanned<LeanStr>>,
    /// Option list for the `::view-transition-group` pseudo-element.
    pub group: Option<Spanned<LeanStr>>,
    /// Utility list for the `::view-transition-old` pseudo-element.
    pub old: Option<Spanned<LeanStr>>,
    /// Utility list for the `::view-transition-new` pseudo-element.
    pub new: Option<Spanned<LeanStr>>,
    /// Utility list for the `::view-transition-image-pair` pseudo-element.
    pub image_pair: Option<Spanned<LeanStr>>,
}

/// A declaration identity paired with source provenance at its use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetReference {
    /// Original UTF-8 declaration identity in its own module.
    pub identity: IdentityId,
    /// Use-site UTF-8 span in the current module; mapped from analysis to original
    /// before compilation.
    pub span: Span,
    /// Authored label of the referenced declaration, when the frontend could read it.
    ///
    /// A label participates in the declaration's generated name, so a use site in
    /// another module needs it to spell the same name the declaring module writes.
    #[serde(default)]
    pub label: Option<Spanned<LeanStr>>,
}

/// How one view-transition use names the participants it marks.
///
/// A view-transition-name is a pairing key rather than an allocated name: different
/// transitions may compose the same value on purpose, so the compiler derives it from the
/// class token and the key instead of checking it against a namespace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ViewTransitionName {
    /// No name: the use marks its participants with a class only.
    None,
    /// A key the use site states.
    Key(ViewTransitionKey),
}

/// One key a view-transition use pairs its participants under.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ViewTransitionKey {
    /// A literal key: the name becomes a static declaration.
    Literal(LeanStr),
    /// An expression only the runtime knows: the name becomes an inline style value.
    Dynamic(Span),
}
