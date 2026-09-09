use std::collections::BTreeMap;

use serde::Serialize;

use stillwind_source::{CompileError, LintFinding, SourceFile};

use crate::compiler::react::props::GeneratedClass;
use crate::transforms::{PropsTransform, TransformError, apply_transforms};

use super::symbols::GeneratedSymbol;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, TransformError> {
        apply_transforms(source, &self.transforms)
    }
}
