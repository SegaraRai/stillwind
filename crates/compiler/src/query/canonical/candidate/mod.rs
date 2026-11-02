//! Finds the canonical spelling of one candidate.
//!
//! Modules:
//!
//! - [`equivalence`] checks that a replacement produces the same CSS as the original
//!   candidate.
//! - [`functions`] suggests replacements for legacy syntax.
//! - [`named`] looks for named values, such as `w-4`.
//! - [`search`] runs the search and returns the result.
//! - [`spelling`] suggests replacements for values and modifiers.
//! - [`variants`] suggests replacements for variants.

mod equivalence;
mod functions;
mod named;
mod search;
mod spelling;
mod variants;

pub(in crate::query::canonical) use equivalence::{authored_variables, variable_calls};
