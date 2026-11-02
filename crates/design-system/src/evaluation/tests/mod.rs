//! Evaluation test suites, one module per subject.
//!
//! Each suite parses real registry CSS, directly or through a snapshot built from CSS
//! in memory, and checks the resolved values, because most evaluation contracts are
//! about what users write instead of internal types. Shared fixtures may expand package
//! CSS from disk. The tests of the loading module cover the caller-driven protocol.
//!
//! Modules:
//!
//! - [`animations`] checks placeholder and value-domain families through the package's
//!   definitions.
//! - [`custom_variants`] checks user-authored variants and the CSS value functions that
//!   they use.
//! - [`overloads`] checks overloaded CSS definitions and compatibility output.
//! - [`registry`] checks registry metadata as the source of resolution behavior.
//! - [`relationships`] checks conditional declarations, relationships, and shared value
//!   families.
//! - [`support`] constructs themes and provides CSSWG syntax helpers.
//! - [`utility_domains`] checks resolution coverage of the upstream utility catalog.
//! - [`value_families`] checks theme values, keywords, fractions, and arbitrary values.
//! - [`variants`] checks variant resolution across the package's variants.

use std::collections::BTreeSet;

use stillwind_css::ValueType;
use stillwind_source::{FileId, SourceFile};
use stillwind_syntax::parse_candidate;

use super::*;
use crate::ThemeIndex;

mod support;
use support::{
    direct_value_domains, expanded_csswg_syntax, formal_terms, has_complex_csswg_grammar,
    resolve_utility, theme,
};

mod animations;
mod custom_variants;
mod overloads;
mod registry;
mod relationships;
mod utility_domains;
mod value_families;
mod variants;
