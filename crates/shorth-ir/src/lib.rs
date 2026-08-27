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
    pub handle_declarations: Vec<Span>,
}

impl ModuleIr {
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            style_calls: Vec::new(),
            handle_declarations: Vec::new(),
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
        handle: Spanned<String>,
        selector: Spanned<String>,
        value: Spanned<String>,
    },
    ConditionalHandleSelectorUtilityString {
        handle: Spanned<String>,
        selector: Spanned<String>,
        guard: Span,
        when_true: bool,
        value: Spanned<String>,
    },
    HandleProjection {
        handle: Spanned<String>,
        projection: HandleProjection,
    },
    ContainerQuery {
        handle: Spanned<String>,
        query: Spanned<String>,
        value: Spanned<String>,
    },
}
