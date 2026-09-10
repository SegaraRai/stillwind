//! Framework policy, independent of parsers and host converters.

use serde::{Deserialize, Serialize};
use stillwind_source::Span;

mod astro;
mod preact;
mod react;
mod solid;
mod svelte;
mod vue;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdapterKind {
    React,
    Preact,
    Solid,
    Vue,
    Svelte,
    Astro,
}

pub struct FrameworkSpec {
    pub id: AdapterKind,
    pub macro_import: &'static str,
    pub runtime_import: &'static str,
    pub class_property: &'static str,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
    pub normalize: fn(ProjectionExpression<'_>) -> ProjectionDecision,
}

/// Parser-neutral view of a possible converter-generated expression wrapper.
#[derive(Clone, Copy, Debug)]
pub enum ProjectionExpression<'a> {
    Member {
        receiver: &'a str,
        whole: Span,
        property: Span,
        optional: bool,
    },
    ExpandedShorthand {
        receiver: &'a str,
        whole: Span,
        key: Span,
        key_name: &'a str,
        value_name: &'a str,
    },
}

pub enum ProjectionDecision {
    Preserve,
    Retain(Span),
}

fn preserve(_: ProjectionExpression<'_>) -> ProjectionDecision {
    ProjectionDecision::Preserve
}

pub const FRAMEWORKS: &[&FrameworkSpec] = &[
    &react::SPEC,
    &preact::SPEC,
    &solid::SPEC,
    &vue::SPEC,
    &svelte::SPEC,
    &astro::SPEC,
];

impl AdapterKind {
    pub const fn spec(self) -> &'static FrameworkSpec {
        match self {
            Self::React => &react::SPEC,
            Self::Preact => &preact::SPEC,
            Self::Solid => &solid::SPEC,
            Self::Vue => &vue::SPEC,
            Self::Svelte => &svelte::SPEC,
            Self::Astro => &astro::SPEC,
        }
    }
}

pub fn from_macro_import(source: &str) -> Option<&'static FrameworkSpec> {
    FRAMEWORKS
        .iter()
        .copied()
        .find(|spec| spec.macro_import == source)
}
