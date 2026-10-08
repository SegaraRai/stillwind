//! Source analyses for project session integration tests.

use std::sync::Arc;

use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, ModuleId, SourceFile};

/// Returns the Oxc analysis of `source` in the module named `name`.
/// Uses file ID 1, TSX syntax, unambiguous module detection, and no external type facts.
///
/// # Panics
///
/// Panics if the source exceeds the source coordinate limit.
pub(crate) fn analysis(name: &str, source: &str) -> ModuleAnalysis {
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
pub(crate) fn module(name: &str, candidate: &str) -> ModuleAnalysis {
    analysis(
        name,
        &format!("import {{ style }} from 'stillwind/react'; const x = style({candidate:?});"),
    )
}
