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
//! definitions hide. [`bypassed`] finds definitions that reach a registry context
//! despite its earlier undefinitions. [`skipped`] reports the imports and directives
//! that loading skipped. [`theme_values`] checks `@theme` token values, and
//! [`theme_references`] checks `theme()` and `--theme()` calls and the theme namespaces
//! that `@utility` definitions read. [`tailwind_features`] reports the Tailwind
//! features that loading ignores. [`variant_directive`] finds nested rules and
//! conditional at-rules that `@variant` can replace, using the reverse index of
//! [`variant_index`] and the comparable header forms of [`headers`].

mod analysis;
mod application;
mod bypassed;
mod definitions;
mod headers;
mod nesting;
mod shadowing;
mod skipped;
mod tailwind_features;
mod theme_references;
mod theme_values;
mod variant;
mod variant_directive;
mod variant_index;

pub use analysis::{StylesheetLint, lint_stylesheets};

#[cfg(test)]
mod tests;
