//! Lowering of the expressions a `style()` call accepts.
//!
//! The mapping is structure-preserving: a conditional stays a conditional, an array
//! stays an array, a variant wrapper stays a wrapper, and each authored expression is
//! lowered exactly once. That is what lets the compiler emit one JavaScript expression
//! per user expression with short-circuiting intact.
//!
//! Anything the frontend cannot resolve locally is not guessed: the expression is
//! recorded as a type probe (when a checker could answer) or reported as a diagnostic
//! (when it cannot be a class value at all). Every produced span is a span of the
//! analysed source.
//!
//! `input` dispatches style arguments and checker probes. `class` lowers class and
//! object expressions. `variant` validates inputs allowed beneath CSS variants.

mod class;
mod input;
mod variant;
