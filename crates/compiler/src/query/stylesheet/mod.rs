//! Editor queries and sorting edits for authored CSS.
//!
//! When the original CSS cannot compile, recovered definitions still support
//! completion, hover, and color queries. Structural sorting and lint fixes require the
//! original CSS graph.
//!
//! Modules:
//!
//! - [`colors`] resolves utility colors and theme references.
//! - [`completion`] implements [`DesignSystemQuery::complete_stylesheet`], owns its
//!   replacement labels, and owns the discovery options that are independent of
//!   compilation.
//! - [`context`] locates cursor inputs through the shared CSS parser and lexical
//!   helpers.
//! - [`directives`] supplies dialect names, syntax, audience, provenance, and usage
//!   positions.
//! - [`hover`] implements [`DesignSystemQuery::hover_stylesheet`] and owns its previews.
//! - [`plugins`] extracts plugin strings and cursor prefixes without loading the CSS
//!   graph.
//! - [`recovery`] extracts definitions for editor queries when the original CSS cannot
//!   compile.
//! - [`sorting`] owns the sorting targets and plans the source edits that stylesheet
//!   lint shares.
//!
//! [`DesignSystemQuery::complete_stylesheet`]: super::DesignSystemQuery::complete_stylesheet
//! [`DesignSystemQuery::hover_stylesheet`]: super::DesignSystemQuery::hover_stylesheet

mod colors;
mod completion;
mod context;
mod directives;
mod hover;
mod plugins;
mod recovery;
mod sorting;

pub use colors::StylesheetColor;
pub use completion::{StylesheetCompletion, StylesheetCompletionOptions};
pub use hover::StylesheetHover;
pub use plugins::{StylesheetPluginPath, stylesheet_plugin_paths};
pub use recovery::editor_css_source;
pub use sorting::{StylesheetEdit, StylesheetSortOptions};

#[cfg(test)]
mod tests;
