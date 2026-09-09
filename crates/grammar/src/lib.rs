//! Stillwind's small, source-spanned utility-string lexer.

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
