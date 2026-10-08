//! Static framework specifications and adapter selection.

use serde::{Deserialize, Serialize};

use super::{ProjectionDecision, ProjectionExpression, astro, preact, react, solid, svelte, vue};

/// Every known framework, in the order hosts use to offer choices.
pub const FRAMEWORKS: &[&FrameworkSpec] = &[
    &react::SPEC,
    &preact::SPEC,
    &solid::SPEC,
    &vue::SPEC,
    &svelte::SPEC,
    &astro::SPEC,
];

/// Identifies the framework adapter a module is compiled for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    /// React.
    React,
    /// Preact.
    Preact,
    /// Solid.
    Solid,
    /// Vue, including single-file components.
    Vue,
    /// Svelte.
    Svelte,
    /// Astro.
    Astro,
}

impl AdapterKind {
    /// Static description of this adapter. Never fails and never allocates.
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

/// Static policy for one framework.
pub struct FrameworkSpec {
    /// The adapter this specification describes; used to close the loop back from a
    /// `SPEC` constant to its `AdapterKind`.
    pub id: AdapterKind,
    /// Module specifier a user imports the `style` macro from, for example
    /// `stillwind/react`. It is the import the compiler may remove.
    pub macro_import: &'static str,
    /// Module that provides the fragment runtime for lowered calls.
    pub runtime_import: &'static str,
    /// Module that provides the development-only helpers, used when the compilation
    /// runs in development mode.
    ///
    /// It is an ordinary specifier without an export condition: the mode is decided by
    /// the compiler, so a condition here would resolve twice, once for the module and
    /// once for the name.
    pub runtime_import_dev: &'static str,
    /// Property name the framework expects for classes: `className` or `class`.
    pub class_property: &'static str,
    /// Property name the framework expects for inline styles. `style` for now.
    pub style_property: &'static str,
    /// How the framework runtime materializes the style payload of a fragment.
    pub style_format: StyleFormat,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
    /// Policy for converter-generated expressions and helper bindings in projected
    /// input.
    pub normalize: fn(ProjectionExpression<'_>) -> ProjectionDecision,
}

/// How a framework runtime accepts generated custom CSS properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleFormat {
    /// React, Preact, Solid, and Vue receive custom CSS properties as an object.
    Object,
    /// Svelte and Astro receive them as a serialized style string.
    String,
}

/// Finds the framework that owns a macro import specifier.
pub fn from_macro_import(source: &str) -> Option<&'static FrameworkSpec> {
    FRAMEWORKS
        .iter()
        .copied()
        .find(|spec| spec.macro_import == source)
}

/// Default host-facing request for an explicit module stylesheet value.
/// Bundler-specific CSS requests and query conventions are separate from this default.
pub const MODULE_CSS_IMPORT: &str = "stillwind/module-css";
