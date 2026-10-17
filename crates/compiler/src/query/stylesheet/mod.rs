//! Completion and hover for authored CSS, including incomplete cursor inputs.
//!
//! [`completion`] implements [`crate::query::DesignSystemQuery::complete_stylesheet`]
//! and owns its replacement labels. [`hover`] implements
//! [`crate::query::DesignSystemQuery::hover_stylesheet`] and owns its previews.
//! [`context`] locates cursor inputs through the shared CSS parser and lexical helpers.
//! [`directives`] supplies dialect names, syntax, and descriptions. [`recovery`]
//! extracts definitions for editor queries when the original CSS cannot compile.
//! Recovered sources must only be used for editor queries.

mod completion;
mod context;
mod directives;
mod hover;
mod recovery;

pub use completion::StylesheetCompletion;
pub use hover::StylesheetHover;
pub use recovery::editor_css_source;

#[cfg(test)]
mod tests;
