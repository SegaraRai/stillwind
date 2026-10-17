//! Editor queries and sorting edits for authored CSS.
//!
//! [`completion`] implements [`crate::query::DesignSystemQuery::complete_stylesheet`]
//! and owns its replacement labels. [`hover`] implements
//! [`crate::query::DesignSystemQuery::hover_stylesheet`] and owns its previews.
//! [`context`] locates cursor inputs through the shared CSS parser and lexical helpers.
//! [`directives`] supplies dialect names, syntax, and descriptions. [`recovery`]
//! extracts definitions for editor queries when the original CSS cannot compile.
//! Recovered definitions support completion, hover, and color queries on incomplete
//! input. Structural sorting and lint fixes require the original CSS graph.
//! [`colors`] resolves utility colors and theme references. [`sorting`] owns the
//! independent sorting targets and plans source edits shared with stylesheet lint.

mod colors;
mod completion;
mod context;
mod directives;
mod hover;
mod recovery;
mod sorting;

pub use colors::StylesheetColor;
pub use completion::StylesheetCompletion;
pub use hover::StylesheetHover;
pub use recovery::editor_css_source;
pub use sorting::{StylesheetEdit, StylesheetSortOptions};

#[cfg(test)]
mod tests;
