//! Stillwind's small, source-spanned utility-string lexer.

mod lexer;
mod syntax;

pub use lexer::{UtilityToken, lex_utilities};
pub use syntax::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};
