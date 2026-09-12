//! Numeric folding for `calc()`, and the printer for folded numbers.
//!
//! The parser builds an expression tree of numbers, dimensions, percentages and opaque
//! atoms; folding combines only what is exactly combinable, and the serializer reprints
//! the rest with the parentheses the tree implies. Nothing here converts units or
//! guesses at unknown functions: an unparsable or unsupported expression is left
//! byte-for-byte as authored.
//!
//! Only [`fold_calc_expressions`], [`fold_calc_expressions_with_zero_unit`] and
//! [`format_numeric_zero`] are re-exported; the tree and parser stay crate-internal.

pub(super) mod expression;
pub(super) mod folding;
pub(super) mod parser;

pub use folding::{
    fold_calc_expressions, fold_calc_expressions_with_zero_unit, format_numeric_zero,
};
