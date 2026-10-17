//! Stylesheet assembly.
//!
//! [`template::render_mapped`] assembles a document or carrier from structured producer
//! contributions and an immutable design system, and [`template::render_css_template`]
//! places one `style.css()` call into a named template. [`render`] exposes text-only
//! wrappers and the assembly error type. The template owns output order, selectors,
//! layers, and conditions; these modules do not resolve host URLs or minify the
//! assembled CSS.
//!
//! [`authored_directives`] expands `@apply` and `@variant`, returning CSS and
//! accumulating registration dependencies. [`properties`] selects registration rules
//! and fallback declarations. [`keyframes`] selects definitions from authored theme
//! text; generated animations already carry their destination. [`template`] combines
//! those contributions while retaining source mappings. [`lint`] checks the marked
//! stylesheets of a snapshot and finds directives through the same structure model as
//! [`authored_directives`].

mod authored_directives;
mod keyframes;
mod lint;
mod properties;
mod render;
mod template;

pub use lint::{StylesheetLint, lint_stylesheets};

pub(crate) use render::{render_module_stylesheet, render_stylesheet};
pub(crate) use template::{render_css_template, render_mapped};

#[cfg(test)]
pub(super) use authored_directives::render_authored_variant_path;
