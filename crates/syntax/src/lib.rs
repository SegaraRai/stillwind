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
//!
//! `lexer` locates utility tokens and their source spans. `syntax` parses each
//! candidate's utility and variant structure without resolving registry names.
//! `candidate` shares delimiter rules with unfinished prefixes and source scanning.
//! `arbitrary` decodes arbitrary values as Tailwind does.

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

mod arbitrary;
mod candidate;
mod lexer;
mod syntax;

pub use arbitrary::decode_arbitrary_value;
pub use candidate::{CandidatePrefix, source_candidate_tokens};
pub use lexer::{UtilityToken, lex_utilities};
pub use syntax::{
    Candidate, Utility, Value, Variant, VariantKind, parse_candidate, parse_variant_prefix,
    utility_registration_key,
};
