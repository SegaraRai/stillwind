//! Tests for canonical spellings and for the canonical index.

use std::sync::Arc;

use lean_string::ToLeanStr;

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CanonicalUtilitiesOverride, LintOverrides, LintSeverity};
use stillwind_source::{CssSourceName, FileId};
use stillwind_test_support::css_source;

use super::index::{policy, rank};
use super::preview::convert_pixel_dimensions;
use crate::query::QueryContext;
use crate::{CompilerOptions, DesignSystemQuery, RootFontSize, prepare_canonical_index};

fn make_query(css_text: &str, options: CompilerOptions) -> DesignSystemQuery {
    let source = css_source(FileId(1), css_text).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("theme.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    DesignSystemQuery::new(snapshot, options)
}

mod index;
mod spelling;
