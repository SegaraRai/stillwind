//! Framework policy, independent of parsers and host converters.
//!
//! Each framework states the same facts: which module exports the macro, which one
//! provides the runtime, what the class attribute is called, how the runtime
//! materializes the style payload, and whether the framework's converter needs
//! original-source projection.
//!
//! The `astro`, `preact`, `react`, `solid`, `svelte`, and `vue` modules supply static
//! specifications and expression-rewrite policies. [`AdapterKind::spec`] selects a
//! specification; [`FRAMEWORKS`] lists them in the order offered to hosts. Supporting
//! another adapter requires its specification and enum/list entries, not a parser here.
//! Source parsing, projection bookkeeping, and host conversion belong to the frontend
//! and host layers.

use serde::{Deserialize, Serialize};

use stillwind_source::Span;

mod astro;
mod preact;
mod react;
mod solid;
mod svelte;
mod vue;

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
    /// Attribute name the framework expects for classes: `className` or `class`.
    pub class_property: &'static str,
    /// How the framework runtime materializes the style payload of a fragment.
    pub style_format: StyleFormat,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
    /// Rewrite policy for converter-generated wrappers, applied when a projected input
    /// is analyzed.
    pub normalize: fn(ProjectionExpression<'_>) -> ProjectionDecision,
}

/// How a framework runtime accepts generated custom CSS properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StyleFormat {
    /// React, Preact, Solid, and Vue receive custom CSS properties as an object.
    Object,
    /// Svelte and Astro receive them as a serialized style string.
    String,
}

/// Parser-neutral view of a possible converter-generated expression wrapper.
///
/// The frontend reports what it saw; the framework's [`FrameworkSpec::normalize`]
/// decides whether the wrapper is scaffolding that may be discarded, a mapping that
/// must be retained, or user code that must be preserved.
#[derive(Clone, Copy, Debug)]
pub enum ProjectionExpression<'a> {
    /// A type alias injected by a language tool. `definition` is its text.
    SyntheticTypeAlias {
        /// Name of the injected alias, for example `__VLS_SetupExposed`.
        name: &'a str,
        /// Text of the alias definition.
        definition: &'a str,
    },
    /// A synthetic reference list emitted next to a binding.
    SyntheticReferenceList {
        /// Comment line the converter placed before the list.
        preceding_line: &'a str,
        /// Name of the binding that follows the list.
        next_binding: &'a str,
    },
    /// `receiver.property`, possibly optional (`receiver?.property`).
    Member {
        /// Text of the receiver expression.
        receiver: &'a str,
        /// Span of the whole member expression in the analyzed buffer.
        whole: Span,
        /// Span of the property name, which is the part the user wrote.
        property: Span,
        /// Whether the access used optional chaining.
        optional: bool,
    },
    /// A shorthand property that a converter expanded into `{ a: a }`; `key` and
    /// `value` are the two occurrences.
    ExpandedShorthand {
        /// Text of the receiver expression.
        receiver: &'a str,
        /// Span of the whole expanded property.
        whole: Span,
        /// Span of the key occurrence, which is the part the user wrote.
        key: Span,
        /// Key text as written.
        key_name: &'a str,
        /// Value text as written; equal to `key_name` for a true shorthand.
        value_name: &'a str,
    },
}

/// What the frontend should do with a candidate wrapper expression.
pub enum ProjectionDecision {
    /// Keep the expression as user-authored code.
    Preserve,
    /// Treat [`Span`] as the user's original expression and drop the wrapper.
    Retain(Span),
    /// Discard only if the caller proves the scaffolding's syntax is unmapped.
    DiscardScaffolding,
}

/// The policy for a framework whose converter never rewrites user code.
fn preserve(_: ProjectionExpression<'_>) -> ProjectionDecision {
    ProjectionDecision::Preserve
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
