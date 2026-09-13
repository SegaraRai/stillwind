//! Evaluation test suites, one module per subject.
//!
//! Each suite parses real registry CSS, either directly or through a snapshot built
//! from CSS that is already in memory, and asserts the resolved values, because most
//! evaluation contracts are about what users write rather than about internal types.
//! Shared fixtures may expand package CSS from disk; `tests/loading.rs` also exercises
//! caller-driven filesystem loading. [`support`] holds the shared fixtures; the other
//! modules own one area each: built-in registry contents, utility matching and domains,
//! value families, variants and custom variants, overloads, relationships, animations,
//! and the registry audit.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use stillwind_css::ValueType;
use stillwind_source::{FileId, SourceFile};
use stillwind_syntax::{Utility, VariantKind, parse_candidate};

use super::utilities::body::resolve_css_utility_body;
use super::utilities::matching::resolve_css_utility_branches;
use super::values::theme::theme_arbitrary_types;
use super::variants::substitution::registered_variant_candidates;
use super::*;
use crate::{CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace};

mod support;
use support::*;

macro_rules! assert_css_eq {
    ($actual:expr, $expected:expr $(,)?) => {
        assert_eq!(css_tokens(&$actual), css_tokens(&$expected));
    };
}

mod animations;
mod audit;
mod custom_variants;
mod overloads;
mod registry;
mod relationships;
mod utility_domains;
mod value_families;
mod variants;
