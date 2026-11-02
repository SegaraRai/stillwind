//! Numeric folding for `calc()`, and the printer for folded numbers.
//!
//! The parser builds an expression tree of numbers, dimensions, percentages, and opaque
//! atoms. Folding combines only terms that are exactly combinable, and the serializer
//! prints the rest with the parentheses that the tree implies. Nothing here converts
//! units or guesses the meaning of unknown functions: an expression that cannot be
//! parsed or is not supported stays byte for byte as authored.
//!
//! Modules:
//!
//! - [`evaluation`] computes one inexact number for editor displays, including
//!   divisions that folding keeps.
//! - [`expression`] owns the expression tree and its printer.
//! - [`folding`] finds `calc()` calls, evaluates compatible terms, and leaves
//!   unsupported expressions unchanged.
//! - [`parser`] reads numeric expressions and enforces the nesting limit.

mod evaluation;
mod expression;
mod folding;
mod parser;

pub use evaluation::evaluate_numeric;
pub use folding::{fold_calc_expressions, format_numeric_zero};
pub use parser::MAX_VALUE_NESTING;
