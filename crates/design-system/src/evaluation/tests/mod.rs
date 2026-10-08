//! Evaluation test suites, one module per subject.
//!
//! Each suite parses real registry CSS, either directly or through a snapshot built
//! from CSS that is already in memory, and asserts the resolved values, because most
//! evaluation contracts are about what users write rather than about internal types.
//! Shared fixtures may expand package CSS from disk; `tests/loading.rs` also exercises
//! caller-driven filesystem loading. [`support`] holds the shared fixtures; the other
//! modules own one area each: built-in registry contents, utility matching and domains,
//! value families, variants and custom variants, overloads, relationships, and animations.

use std::collections::BTreeSet;

use stillwind_css::ValueType;
use stillwind_source::{FileId, SourceFile};
use stillwind_syntax::parse_candidate;

use super::values::theme_arbitrary_types;
use super::*;
use crate::{ThemeIndex, ThemeNamespace};

mod support;
use support::{
    direct_value_domains, expanded_csswg_syntax, formal_terms, has_complex_csswg_grammar,
    resolve_animation_option, resolve_utility, theme,
};

mod animations;
mod custom_variants;
mod overloads;
mod registry;
mod relationships;
mod utility_domains;
mod value_families;
mod variants;
