//! Ordered style inputs and the ref conditions and variants they apply.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_semantic::IdentityId;
use stillwind_source::{Span, Spanned};

use super::{
    AssetReference, ClassExpression, ConditionalBranches, RefDefinition, ViewTransitionName,
};

/// One `style(...)` call with its evaluated inputs in source order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleCall {
    /// Span of the whole call, including the callee expression.
    pub span: Span,
    /// Arguments in source order. The base unit of a rule is built from the
    /// unconditional entries; conditional entries follow in this order.
    pub inputs: Vec<StyleInput>,
}

/// An element condition over the finite refs that one expression may denote.
///
/// A scope operand carries the same condition without introducing another scope or a
/// container query. The prefix is resolved against each referenced element.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefCondition {
    /// Finite declaration candidates. Empty candidates are diagnosed during lowering.
    pub refs: Vec<RefDefinition>,
    /// Static variant text with an optional final separator normalized to `:`. Empty
    /// text tests marker presence. The span identifies this condition's use.
    pub prefix: Spanned<LeanStr>,
}

/// The CSS role a ref plays at a use site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum RefProjection {
    /// Supplies the receiver's anchor name and optional explicit scope.
    AsAnchor(
        /// `None` omits `anchor-scope`. An explicit list includes the receiver only
        /// when it occurs in that list.
        Option<AnchorScope>,
    ),
    /// Supplies the receiver's container name and optional container type.
    AsContainer(
        /// A supported literal in normalized form, with its source span. `None` omits
        /// `container-type`. The compiler does not provide a default.
        Option<Spanned<ContainerType>>,
    ),
    /// References an anchor from the styled element.
    PositionAnchor,
}

/// A supported CSS container type, with combined keywords in canonical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContainerType {
    /// Serialized as `normal`.
    #[serde(rename = "normal")]
    Normal,
    /// Serialized as `size`.
    #[serde(rename = "size")]
    Size,
    /// Serialized as `inline-size`.
    #[serde(rename = "inline-size")]
    InlineSize,
    /// Serialized as `scroll-state`.
    #[serde(rename = "scroll-state")]
    ScrollState,
    /// Serialized as `size scroll-state`.
    #[serde(rename = "size scroll-state")]
    SizeScrollState,
    /// Serialized as `inline-size scroll-state`.
    #[serde(rename = "inline-size scroll-state")]
    InlineSizeScrollState,
    /// Serialized as `initial`.
    #[serde(rename = "initial")]
    Initial,
    /// Serialized as `inherit`.
    #[serde(rename = "inherit")]
    Inherit,
    /// Serialized as `unset`.
    #[serde(rename = "unset")]
    Unset,
    /// Serialized as `revert`.
    #[serde(rename = "revert")]
    Revert,
    /// Serialized as `revert-layer`.
    #[serde(rename = "revert-layer")]
    RevertLayer,
    /// Serialized as `revert-rule`.
    #[serde(rename = "revert-rule")]
    RevertRule,
}

impl ContainerType {
    /// Parses a trimmed CSS literal. Internal spacing must match a supported spelling.
    /// Returns `None` for unsupported text. Combined keywords accept either order.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "normal" => Self::Normal,
            "size" => Self::Size,
            "inline-size" => Self::InlineSize,
            "scroll-state" => Self::ScrollState,
            "size scroll-state" | "scroll-state size" => Self::SizeScrollState,
            "inline-size scroll-state" | "scroll-state inline-size" => Self::InlineSizeScrollState,
            "initial" => Self::Initial,
            "inherit" => Self::Inherit,
            "unset" => Self::Unset,
            "revert" => Self::Revert,
            "revert-layer" => Self::RevertLayer,
            "revert-rule" => Self::RevertRule,
            _ => return None,
        })
    }

    /// Returns the CSS spelling with combined keywords in canonical order.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Size => "size",
            Self::InlineSize => "inline-size",
            Self::ScrollState => "scroll-state",
            Self::SizeScrollState => "size scroll-state",
            Self::InlineSizeScrollState => "inline-size scroll-state",
            Self::Initial => "initial",
            Self::Inherit => "inherit",
            Self::Unset => "unset",
            Self::Revert => "revert",
            Self::RevertLayer => "revert-layer",
            Self::RevertRule => "revert-rule",
        }
    }
}

impl std::fmt::Display for ContainerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Explicit anchor visibility, resolved to the union of finite ref candidates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum AnchorScope {
    /// The CSS keyword `all`.
    All,
    /// The CSS keyword `none`.
    None,
    /// Unique ref identities whose generated anchor names are scoped.
    Refs(
        /// All possible ref declarations, with each identity included once in
        /// first-seen order. An empty list emits no scope declaration.
        Vec<RefDefinition>,
    ),
}

/// Ordered variants shared by object wrappers and direct template interpolation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StyleVariant {
    /// A normal variant prefix. A nonempty prefix includes its final separator;
    /// an empty prefix contributes no variants. The compiler validates the CSS
    /// variant grammar when resolving the wrapper.
    Prefix(
        /// Authored prefix and its original source span.
        Spanned<LeanStr>,
    ),
    /// A named-container query over finite ref candidates, written as `ref.at(query)`.
    Container {
        /// Each candidate supplies a container identity, independently of the subject
        /// selector. An empty set is diagnosed as an unidentified ref by variant
        /// resolution.
        refs: Vec<RefDefinition>,
        /// Literal CSS query condition, without the generated container name.
        /// Empty text emits only the container name.
        query: Spanned<LeanStr>,
    },
    /// A ref condition with optional shared scopes, written as a call or tag followed
    /// by zero or more `.in(scope)` calls.
    Ref {
        /// The element state or marker presence tested by every shared scope.
        condition: RefCondition,
        /// Independent scope conditions. Each scope must contain the referenced element
        /// and the target. Their order does not define an ancestor path.
        scopes: Vec<RefCondition>,
    },
}

/// One evaluated argument of a `style(...)` call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StyleInput {
    /// Source array whose entries can contribute to multiple props channels.
    Array(
        /// Source elements in evaluation order, possibly empty.
        Vec<Self>,
    ),
    /// Selects complete style contributions, including classes, names, refs, and
    /// composition.
    Conditional {
        /// Original condition expression or lookup key.
        test: Span,
        /// Contributions selected by the source form, in source order.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// `style(base)`: this call derives from another style call's payload.
    /// `candidates` lists declaration identities reachable from the base value so
    /// dependency planning does not re-parse the referenced expression.
    Composition {
        /// The referenced base expression, emitted verbatim.
        value: Span,
        /// Candidate identities of the base call, deduplicated in identity order. An
        /// empty finite set has no static dependency; the base expression is still
        /// passed to the runtime composition path.
        candidates: Vec<IdentityId>,
    },
    /// Class contributions that keep the source expression structure.
    Class(
        /// Root class expression, which may contain props-bearing object entries.
        ClassExpression,
    ),
    /// `style(ref)`: attach a ref's props and marker class to the element.
    ///
    /// Shared scope selectors also use this marker to identify their containing
    /// element.
    RefAttachment {
        /// Ref whose marker and attachment props apply to this element.
        definition: RefDefinition,
        /// Original expression selecting the ref at this use site.
        value: Span,
    },
    /// A ref value that is only known at runtime, with every statically reachable
    /// alternative listed for CSS planning.
    DynamicRefAttachment {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. An empty
        /// finite set contributes no planned ref assets; the runtime class read remains
        /// in the emitted expression.
        refs: Vec<RefDefinition>,
    },
    /// `style(transition)` with a statically known transition.
    ViewTransitionUse {
        /// The declared transition being used.
        transition: AssetReference,
        /// How the use names its participants.
        name: ViewTransitionName,
    },
    /// A transition value that is only known at runtime.
    DynamicViewTransitionUse {
        /// The runtime transition value.
        value: Span,
        /// Every statically reachable alternative. The JavaScript frontend omits this
        /// use when the finite set is empty.
        transitions: Vec<AssetReference>,
        /// How the use names its participants.
        name: ViewTransitionName,
    },
}
