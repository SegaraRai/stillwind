//! Lowering of the expressions that a `style()` call accepts.
//!
//! The mapping preserves structure: a conditional stays a conditional, an array stays
//! an array, a variant wrapper stays a wrapper, and each authored expression is lowered
//! exactly once. The compiler can therefore emit one JavaScript expression per authored
//! expression and keep short-circuiting.
//!
//! The frontend never guesses a value that it cannot resolve locally. It records the
//! expression as a type probe when a checker can answer, or reports a diagnostic when
//! the expression cannot be a class value at all. Every produced span is a span of the
//! analyzed source.
//!
//! Modules:
//!
//! - [`class`] lowers class and object expressions.
//! - [`input`] dispatches style arguments and checker probes.
//! - [`nested`] omits inputs that would keep another macro call in JavaScript.
//! - [`table`] lowers lookups in array and object literals.
//! - [`variant`] validates the inputs allowed beneath CSS variants.

mod class;
mod input;
mod nested;
mod table;
mod variant;
