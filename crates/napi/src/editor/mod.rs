//! Editor requests for JavaScript, answered with the compiler's resolution, lint
//! settings, and reporting.
//!
//! Each module that adds requests also defines the types that those requests exchange
//! with JavaScript.
//!
//! Modules:
//!
//! - [`candidates`] adds candidate requests: canonical spellings, previews, completion,
//!   marker classes, ordering, and colors.
//! - [`lint`] adds requests that report lint findings and check suppression comments.
//! - [`query`] defines [`NativeEditorQuery`], which answers requests for one design
//!   system.
//! - [`stylesheet`] adds requests for authored stylesheets.
//! - [`theme`] adds requests for the theme tokens and shared assets that JavaScript
//!   plugins read.

mod candidates;
mod lint;
mod query;
mod stylesheet;
mod theme;

pub use candidates::{NativeCandidateContext, decode_arbitrary_value};
pub use lint::NativeEditorLintEdit;
pub use query::NativeEditorQuery;
pub use stylesheet::{
    NativeStylesheetPluginPath, completion_detail, editor_css_source, stylesheet_plugin_paths,
};
