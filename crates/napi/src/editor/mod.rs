//! Provides editor requests to JavaScript. The requests use the compiler's resolution,
//! lint settings, and reporting.
//!
//! `query` defines `NativeEditorQuery`, which answers requests for one design system.
//! `candidates`, `stylesheet`, `lint`, and `theme` add the requests of each area to
//! that type, together with the types that those requests exchange with JavaScript.

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
