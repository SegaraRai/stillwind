//! Source-spanned lexer and candidate parser for Stillwind utility strings.
//!
//! The crate owns the syntax layer only. [`lex_utilities`] splits one decoded utility
//! string into tokens that keep their source spans. [`parse_candidate`] splits one
//! token into variants, importance, negativity, and a utility. Candidate parsing is
//! lossless: [`Candidate::raw`] keeps the exact source spelling for hashing and
//! diagnostics, and decoded arbitrary values keep their decoded text for evaluation.
//!
//! The grammar never validates names against a registry, because only the design system
//! knows whether `bg-red-500` is a real utility. Order is always source order, and
//! nothing here performs I/O.
//!
//! Modules:
//!
//! - [`arbitrary`] decodes and prints arbitrary values as Tailwind does.
//! - [`candidate`] shares delimiter rules between unfinished prefixes and source
//!   scanning.
//! - [`lexer`] locates utility tokens and their source spans.
//! - [`mod@syntax`] parses the utility and variant structure of each candidate without
//!   resolving registry names.

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

pub use arbitrary::{decode_arbitrary_value, print_arbitrary_value};
pub use candidate::{CandidatePrefix, source_candidate_tokens};
pub use lexer::{UtilityToken, lex_utilities};
pub use syntax::{
    Candidate, Utility, Value, Variant, VariantKind, parse_candidate, parse_variant_prefix,
    utility_registration_key,
};
