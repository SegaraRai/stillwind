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
}

impl ModuleIr {
    pub const fn react() -> Self {
        Self {
            adapter: AdapterKind::React,
            style_calls: Vec::new(),
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
pub enum StyleInput {
    UtilityString(Spanned<String>),
    ConditionalUtilityString { guard: Span, value: Spanned<String> },
    Attributes(Vec<AttributeEntry>),
}
