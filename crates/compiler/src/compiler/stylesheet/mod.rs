//! Stylesheet assembly.
//!
//! [`template::render_mapped`] assembles a document or carrier from structured producer
//! contributions and an immutable design system. [`template::render_css_template`]
//! places one `style.css()` call into a named template. The template owns output order,
//! selectors, layers, and conditions. These modules do not resolve host URLs or minify
//! the assembled CSS.
//!
//! Modules:
//!
//! - [`authored_directives`] expands `@apply` and `@variant` and collects registration
//!   dependencies.
//! - [`keyframes`] selects keyframes definitions from authored theme text. Generated
//!   animations already carry their destination.
//! - [`lint`] checks the marked stylesheets of a snapshot through the same structure
//!   model as [`authored_directives`].
//! - [`properties`] selects registration rules and fallback declarations.
//! - [`render`] provides raw-CSS wrappers that keep notices, and the assembly error
//!   type.
//! - [`template`] combines the contributions and keeps their source mappings.
//! - [`theme_prefix`] applies the configured property prefixes to CSS identifiers.

mod authored_directives;
mod keyframes;
mod lint;
mod properties;
mod render;
mod template;
mod theme_prefix;

pub use lint::{StylesheetLint, lint_stylesheets};

pub(crate) use authored_directives::{
    applied_banners, apply_candidates, expand_authored_variant, parse_variant_sequence,
    variant_sequences,
};
pub(crate) use render::{render_module_stylesheet, render_stylesheet};
pub(crate) use template::{render_css_template, render_mapped};
pub(crate) use theme_prefix::{prefix_theme_variables, rewrite_generated_css};

pub(super) use authored_directives::expand_authored_directives;
pub(super) use authored_directives::fold_nested_host_rules;
pub(super) use authored_directives::render_authored_variant_steps;

#[cfg(test)]
pub(super) use authored_directives::render_authored_variant_path;
