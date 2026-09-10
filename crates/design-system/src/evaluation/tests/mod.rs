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

mod animations;
mod audit;
mod custom_variants;
mod overloads;
mod registry;
mod relationships;
mod utility_domains;
mod value_families;
mod variants;
