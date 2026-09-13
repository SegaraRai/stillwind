//! Stylesheet assembly.
//!
//! [`template::render_mapped`] assembles a document or carrier from structured producer
//! contributions and an immutable design system. [`render`] exposes text-only wrappers
//! and the assembly error type. The template owns output order, selectors, layers, and
//! conditions; these modules do not resolve host URLs or minify the assembled CSS.
//!
//! [`authored_directives`] expands `@apply` and `@variant`, returning CSS and accumulating
//! registration dependencies. [`properties`] selects registration rules and fallback
//! declarations. [`keyframes`] selects definitions from authored theme text; generated
//! animations already carry their destination. [`blocks`] supplies the authored at-rule
//! scanner. [`template`] combines those contributions while retaining source mappings.

pub(super) mod authored_directives;
pub(super) mod blocks;
pub(super) mod keyframes;
pub(super) mod properties;
pub(super) mod render;
pub(super) mod template;
