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
//! checks the sequences of one `@variant` prelude. [`definitions`] finds the definition
//! at-rules of a source, such as `@utility` and `@custom-variant`. [`nesting`] checks
//! variant selectors without `&`, and [`shadowing`] finds definitions that later
//! definitions hide. [`theme_values`] checks `@theme` token values, and
//! [`theme_references`] checks `theme()` and `--theme()` calls and the theme namespaces
//! that `@utility` definitions read.

mod analysis;
mod application;
mod definitions;
mod nesting;
mod shadowing;
mod theme_references;
mod theme_values;
mod variant;

pub use analysis::{StylesheetLint, lint_stylesheets};

#[cfg(test)]
mod tests;
