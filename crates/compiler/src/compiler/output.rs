use std::collections::BTreeMap;

use serde::Serialize;
use stillwind_source::{CompileError, LintFinding, SourceFile};

use super::props::GeneratedClass;
use super::symbols::GeneratedSymbol;
use crate::transforms::{SourceTransform, TransformError, apply_transforms};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub resources: Vec<crate::ModuleResourceReference>,
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<SourceTransform>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

impl CompileResult {
    /// Validates source-spanned macro replacements and renders them in source order.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error for failed compilation, or when a transform targets
    /// another file or overlaps a previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, TransformError> {
        if !self.diagnostics.is_empty() {
            return Err(TransformError::CompilationFailed);
        }
        apply_transforms(source, &self.transforms)
    }
}
