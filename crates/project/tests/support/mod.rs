//! Inputs shared by the project session integration tests.
//!
//! Each integration-test binary uses a subset of these constructors.

#![allow(dead_code)]

use std::sync::Arc;

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};
use stillwind_test_support::css;

/// Returns a default design-system snapshot from `content` and the standard package.
/// The CSS source has file ID 2 and the name `test.css`.
///
/// # Panics
///
/// Panics if the fixture CSS cannot be loaded into a snapshot.
pub fn snapshot(content: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &css::source(FileId(2), content).unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

/// Returns a default snapshot with a `--color-brand` token whose CSS value is `color`.
///
/// # Panics
///
/// Panics if the fixture CSS cannot be loaded into a snapshot.
pub fn brand_snapshot(color: &str) -> DesignSystemSnapshot {
    snapshot(&format!("@theme {{ --color-brand: {color}; }}"))
}

/// Returns the Oxc analysis of `source` in the module named `name`.
/// Uses file ID 1, TSX syntax, unambiguous module detection, and no external type facts.
///
/// # Panics
///
/// Panics if the source exceeds the source coordinate limit.
pub fn analysis(name: &str, source: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(SourceFile::new(FileId(1), None, None, source).unwrap()),
            ModuleId::from(name),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    )
}

/// Returns the analysis of one React style call using `candidate` in module `name`.
/// Source identity and parser settings follow [`analysis`].
///
/// # Panics
///
/// Panics if the generated source exceeds the source coordinate limit.
pub fn module(name: &str, candidate: &str) -> ModuleAnalysis {
    analysis(
        name,
        &format!("import {{ style }} from 'stillwind/react'; const x = style({candidate:?});"),
    )
}
