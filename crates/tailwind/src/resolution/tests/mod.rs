use std::{collections::BTreeSet, fs, path::Path};

use stillwind_css::parse_tailwind_theme;
use stillwind_source::{FileId, SourceFile};

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
