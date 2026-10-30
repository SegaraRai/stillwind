//! Stylesheet assembly.
//!
//! [`template::render_mapped`] assembles a document or carrier from structured producer
//! contributions and an immutable design system, and [`template::render_css_template`]
//! places one `style.css()` call into a named template. [`render`] exposes raw-CSS
//! wrappers that retain notices and the assembly error type. The template owns output
//! order, selectors, layers, and conditions; these modules do not resolve host URLs or
//! minify the assembled CSS.
//!
//! [`authored_directives`] expands `@apply` and `@variant`, returning CSS and
//! accumulating registration dependencies. [`properties`] selects registration rules
//! and fallback declarations. [`keyframes`] selects definitions from authored theme
//! text; generated animations already carry their destination. [`template`] combines
//! those contributions while retaining source mappings. [`theme_prefix`] renames theme
//! variables in finished output. [`lint`] checks the marked stylesheets of a snapshot
//! and finds directives through the same structure model as [`authored_directives`].

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
pub(crate) use theme_prefix::prefix_theme_variables;

pub(super) use authored_directives::expand_authored_directives;

#[cfg(test)]
pub(super) use authored_directives::render_authored_variant_path;
