//! Language-neutral intermediate representation shared by all source frontends.

use serde::{Deserialize, Serialize};
use shorth_source::{Span, Spanned};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdapterKind {
    React,
    Vue,
    Svelte,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleIr {
    pub adapter: AdapterKind,
    pub style_calls: Vec<StyleCall>,
    pub handle_definitions: Vec<HandleDefinition>,
    pub scope_definitions: Vec<ScopeDefinition>,
    pub animation_declarations: Vec<AnimationDefinition>,
    pub view_transition_declarations: Vec<ViewTransitionDefinition>,
}

impl ModuleIr {
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            style_calls: Vec::new(),
            handle_definitions: Vec::new(),
            scope_definitions: Vec::new(),
            animation_declarations: Vec::new(),
            view_transition_declarations: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleCall {
    pub span: Span,
    pub inputs: Vec<StyleInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeEntry {
    pub name: Spanned<String>,
    pub value: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandleDefinition {
    pub name: Spanned<String>,
    pub label: Option<Spanned<String>>,
    pub scope: Option<ScopeDefinition>,
    pub call_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeDefinition {
    pub name: Spanned<String>,
    pub label: Option<Spanned<String>>,
    pub call_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationDefinition {
    pub name: Spanned<String>,
    pub call_span: Span,
    pub frames: Vec<AnimationFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimationFrame {
    pub selector: Spanned<String>,
    pub utilities: Spanned<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTransitionDefinition {
    pub name: Spanned<String>,
    pub call_span: Span,
    pub group: Option<Spanned<String>>,
    pub old: Option<Spanned<String>>,
    pub new: Option<Spanned<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HandleProjection {
    Anchor,
    Position,
    Scope,
    Container(Spanned<String>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleInput {
    UtilityString(Spanned<String>),
    ConditionalUtilityString {
        guard: Span,
        when_true: bool,
        value: Spanned<String>,
    },
    Attributes(Vec<AttributeEntry>),
    SelectorUtilityString {
        selector: Spanned<String>,
        value: Spanned<String>,
    },
    ConditionalSelectorUtilityString {
        selector: Spanned<String>,
        guard: Span,
        when_true: bool,
        value: Spanned<String>,
    },
    HandleSelectorUtilityString {
        handle: HandleDefinition,
        selector: Spanned<String>,
        value: Spanned<String>,
    },
    ConditionalHandleSelectorUtilityString {
        handle: HandleDefinition,
        selector: Spanned<String>,
        guard: Span,
        when_true: bool,
        value: Spanned<String>,
    },
    HandleProjection {
        handle: HandleDefinition,
        projection: HandleProjection,
    },
    ContainerQuery {
        handle: HandleDefinition,
        query: Spanned<String>,
        value: Spanned<String>,
    },
    ScopeAttachment(ScopeDefinition),
    HandleAttachment(HandleDefinition),
    /// A user-owned class value evaluated by the runtime `stillwindClsx` helper.
    ///
    /// This is intentionally available only through `{ class: … }`, so the
    /// surrounding object syntax remains unambiguous for attributes/selectors.
    ExternalClass(Span),
    AnimationUse {
        animation: Spanned<String>,
        options: Spanned<String>,
    },
    SelectorAnimationUse {
        selector: Spanned<String>,
        animation: Spanned<String>,
        options: Spanned<String>,
    },
    ViewTransitionUse {
        transition: Spanned<String>,
        key: Option<Span>,
    },
}
