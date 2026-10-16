//! Lint of the stylesheets that a host marks for linting.
//!
//! [`lint_stylesheets`] checks the marked sources of one design-system snapshot and
//! shares the result between later calls for the same snapshot and effective options.
//! It reads each source through the same structure model as `@apply` and `@variant`
//! expansion. Lint never changes compilation output, and package CSS that the host
//! leaves unmarked is not checked.
//!
//! [`analysis`] walks the rule tree of each source and applies suppression comments.
//! [`application`] checks the candidates of one `@apply` statement, and [`variant`]
//! checks the sequences of one `@variant` prelude. Definitions such as `@utility` and
//! `@custom-variant` are not checked here.

mod analysis;
mod application;
mod variant;

pub use analysis::{StylesheetLint, lint_stylesheets};

#[cfg(test)]
mod tests;
