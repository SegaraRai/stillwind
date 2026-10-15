//! Numeric folding for `calc()`, and the printer for folded numbers.
//!
//! The parser builds an expression tree of numbers, dimensions, percentages and opaque
//! atoms; folding combines only what is exactly combinable, and the serializer reprints
//! the rest with the parentheses the tree implies. Nothing here converts units or
//! guesses at unknown functions: an unparsable or unsupported expression is left
//! byte-for-byte as authored.
//!
//! `parser` reads numeric expressions and enforces the nesting limit. `expression`
//! owns the tree and its printer. `folding` finds `calc()` calls, evaluates compatible
//! terms, and leaves unsupported expressions unchanged. `evaluation` computes one
//! inexact number for editor displays, including divisions that folding keeps.

mod evaluation;
mod expression;
mod folding;
mod parser;

pub use evaluation::evaluate_numeric;
pub use folding::{fold_calc_expressions, format_numeric_zero, value_nesting_overflow};
pub use parser::MAX_VALUE_NESTING;
