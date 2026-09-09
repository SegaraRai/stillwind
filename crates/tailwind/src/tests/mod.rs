use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use stillwind_css::{
    CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace, ValueType, parse_tailwind_theme,
};
use stillwind_grammar::{Utility, VariantKind, parse_candidate};
use stillwind_source::{FileId, SourceFile};

use super::builtins::normalization::canonical_builtin_body;
use super::utilities::body::resolve_css_utility_body;
use super::utilities::matching::resolve_css_utility_branches;
use super::values::theme::theme_arbitrary_types;
use super::variants::substitution::registered_variant_candidates;
use super::*;

mod support;
use support::*;

mod animations;
mod audit;
mod custom_variants;
mod overloads;
mod registry;
mod relationships;
mod utility_domains;
mod value_families;
mod variants;
