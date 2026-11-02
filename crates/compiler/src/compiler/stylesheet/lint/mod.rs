//! Lint of the stylesheets that a host marks for linting.
//!
//! [`lint_stylesheets`] checks the marked sources of one design-system snapshot and
//! shares the result with later calls for the same snapshot and effective options. It
//! reads each source through the same structure model as `@apply` and `@variant`
//! expansion. Lint never changes compilation output, and package CSS that the host
//! leaves unmarked is not checked.
//!
//! Modules:
//!
//! - [`analysis`] walks the rule tree of each source and applies suppression comments.
//! - [`application`] checks the candidates of one `@apply` statement.
//! - [`bypassed`] finds definitions that reach a registry context despite an earlier
//!   undefinition.
//! - [`definitions`] finds the definition at-rules of a source, such as `@utility` and
//!   `@custom-variant`.
//! - [`headers`] builds comparable forms of rule and at-rule headers.
//! - [`nesting`] checks variant selectors that omit `&`.
//! - [`runtime_conditionals`] reports the declarations that rendering leaves out
//!   because their `--if()` cannot be lowered.
//! - [`shadowing`] finds definitions that later definitions hide.
//! - [`skipped`] reports the imports and directives that loading skipped.
//! - [`tailwind_features`] reports the Tailwind features that loading ignores.
//! - [`theme_references`] checks `theme()` and `--theme()` calls and the theme
//!   namespaces that `@utility` definitions read.
//! - [`theme_values`] checks `@theme` token values.
//! - [`variant`] checks the sequences of one `@variant` prelude.
//! - [`variant_directive`] finds nested rules and conditional at-rules that `@variant`
//!   can replace.
//! - [`variant_index`] maps the CSS that `@variant` expands to back to variant
//!   spellings.

mod analysis;
mod application;
mod bypassed;
mod definitions;
mod headers;
mod nesting;
mod runtime_conditionals;
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
