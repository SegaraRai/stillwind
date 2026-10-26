//! Finds the canonical spelling of one candidate.
//!
//! `search` runs the search and returns the result. `equivalence` checks that a
//! replacement produces the same CSS as the original candidate. The other modules
//! suggest replacements: `functions` rewrites legacy syntax, `spelling` rewrites values
//! and modifiers, `named` looks for named values such as `w-4`, and `variants` rewrites
//! variants.

mod equivalence;
mod functions;
mod named;
mod search;
mod spelling;
mod variants;

pub(in crate::query::canonical) use equivalence::{authored_variables, variable_calls};
