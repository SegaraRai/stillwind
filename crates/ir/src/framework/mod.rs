//! Framework policy, independent of parsers and host converters.

use serde::{Deserialize, Serialize};

use stillwind_source::Span;

mod astro;
mod preact;
mod react;
mod solid;
mod svelte;
mod vue;

pub const FRAMEWORKS: &[&FrameworkSpec] = &[
    &react::SPEC,
    &preact::SPEC,
    &solid::SPEC,
    &vue::SPEC,
    &svelte::SPEC,
    &astro::SPEC,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdapterKind {
    React,
    Preact,
    Solid,
    Vue,
    Svelte,
    Astro,
}

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

pub struct FrameworkSpec {
    pub id: AdapterKind,
    pub macro_import: &'static str,
    pub runtime_import: &'static str,
    pub class_property: &'static str,
    /// How the framework runtime materializes the style payload of a fragment.
    pub style_format: StyleFormat,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
    pub normalize: fn(ProjectionExpression<'_>) -> ProjectionDecision,
}

/// The materialized style form of a framework runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleFormat {
    /// React, Preact, Solid, and Vue receive custom properties as an object.
    Object,
    /// Svelte and Astro receive them as a serialized style string.
    String,
}

/// Parser-neutral view of a possible converter-generated expression wrapper.
#[derive(Clone, Copy, Debug)]
pub enum ProjectionExpression<'a> {
    SyntheticTypeAlias {
        name: &'a str,
        definition: &'a str,
    },
    SyntheticReferenceList {
        preceding_line: &'a str,
        next_binding: &'a str,
    },
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
    /// Discard only if the caller proves the scaffolding's syntax is unmapped.
    DiscardScaffolding,
}

fn preserve(_: ProjectionExpression<'_>) -> ProjectionDecision {
    ProjectionDecision::Preserve
}

pub fn from_macro_import(source: &str) -> Option<&'static FrameworkSpec> {
    FRAMEWORKS
        .iter()
        .copied()
        .find(|spec| spec.macro_import == source)
}

/// Default host-facing request for an explicit module stylesheet value.
/// Bundler-specific CSS requests and query conventions are separate from this default.
pub const MODULE_CSS_IMPORT: &str = "stillwind/module-css";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frameworks_declare_their_materialized_style_format() {
        for spec in [&react::SPEC, &preact::SPEC, &solid::SPEC] {
            assert_eq!(
                spec.style_format,
                StyleFormat::Object,
                "{}",
                spec.macro_import
            );
        }
        assert_eq!(vue::SPEC.style_format, StyleFormat::Object);
        assert_eq!(svelte::SPEC.style_format, StyleFormat::String);
        assert_eq!(astro::SPEC.style_format, StyleFormat::String);
        for spec in FRAMEWORKS {
            assert!(spec.class_property == "className" || spec.class_property == "class");
            assert!(spec.runtime_import.ends_with("/runtime"));
        }
    }
}
