//! Static framework specifications and adapter selection.

use serde::{Deserialize, Serialize};

use super::{
    OutputFormat, ProjectionDecision, ProjectionExpression, astro, preact, react, solid, svelte,
    vanilla, vue,
};

/// Every known framework, in the order hosts use to offer choices.
pub const FRAMEWORKS: &[&FrameworkSpec] = &[
    &astro::SPEC,
    &preact::SPEC,
    &react::SPEC,
    &solid::SPEC,
    &svelte::SPEC,
    &vanilla::SPEC,
    &vue::SPEC,
];

/// Identifies a source framework or macro entrypoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    /// Astro.
    Astro,
    /// Preact.
    Preact,
    /// React.
    React,
    /// Solid.
    Solid,
    /// Svelte.
    Svelte,
    /// JavaScript or TypeScript that applies props directly to DOM elements.
    Vanilla,
    /// Vue, including single-file components.
    Vue,
}

impl AdapterKind {
    /// Static description of this adapter. Never fails and never allocates.
    pub const fn spec(self) -> &'static FrameworkSpec {
        match self {
            Self::Astro => &astro::SPEC,
            Self::Preact => &preact::SPEC,
            Self::React => &react::SPEC,
            Self::Solid => &solid::SPEC,
            Self::Svelte => &svelte::SPEC,
            Self::Vanilla => &vanilla::SPEC,
            Self::Vue => &vue::SPEC,
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
    /// Compiled prop representation shared with other compatible entrypoints.
    pub output: OutputFormat,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
    /// Policy for converter-generated expressions and helper bindings in projected
    /// input.
    pub normalize: fn(ProjectionExpression<'_>) -> ProjectionDecision,
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
