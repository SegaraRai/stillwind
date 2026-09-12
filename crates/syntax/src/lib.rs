//! Stillwind's small, source-spanned utility-string lexer and candidate parser.
//!
//! The crate owns the syntax layer only. [`lex_utilities`] turns one decoded utility
//! string into tokens that keep their source spans; [`parse_candidate`] turns one token
//! into variants, importance, negativity and a utility. Candidate parsing is lossless:
//! [`Candidate::raw`] keeps the exact source spelling for hashing and diagnostics,
//! while decoded arbitrary values keep their decoded text for evaluation.
//!
//! The grammar never validates names against a registry, because it cannot: only the
//! design system knows whether `bg-red-500` is a real utility. Order is source order
//! everywhere, and nothing here performs I/O.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod lexer;
mod syntax;

pub use lexer::{UtilityToken, lex_utilities};
pub use syntax::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};
