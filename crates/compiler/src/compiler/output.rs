//! The compilation result shape handed to a host.
//!
//! [`CompileResult`] is the transport boundary. [`CompileResult::module_css`] carries unwrapped
//! utility rules and is skipped by serde, because a host transports the assembled
//! [`CompileResult::css`]; the field stays addressable for a caller that assembles CSS itself.
//! [`CompileResult::apply_to_source`] refuses to render when diagnostics are present, so a host
//! cannot apply a partial edit plan.

use std::collections::BTreeMap;

use serde::Serialize;

use stillwind_ir::StyleDependency;
use stillwind_source::{CompileError, LintFinding, SourceFile};

use super::props::GeneratedClass;
use super::symbols::GeneratedSymbol;
use crate::{
    ModuleResourceReference,
    transforms::{SourceTransform, TransformError, apply_transforms},
};

/// Everything one compilation produced.
///
/// Successful pipeline output contains CSS, generated identifiers and source edits.
/// Marker classes and asset symbols need not own a CSS rule. Failed pipeline output
/// clears generated output; public construction does not enforce that relationship.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    /// Style dependencies a consumer must order this module after.
    pub composition: Vec<StyleDependency>,
    /// Host-resolvable URLs that were replaced by placeholders in the CSS.
    pub resources: Vec<ModuleResourceReference>,
    /// The assembled stylesheet for this compilation.
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    /// Classes generated at this module's callsites, with their guard paths.
    pub classes: Vec<GeneratedClass>,
    /// Generated CSS identifiers with the identities they were allocated from.
    pub symbols: Vec<GeneratedSymbol>,
    /// Replacements to apply to the original source.
    pub transforms: Vec<SourceTransform>,
    /// Errors that failed the compilation.
    pub diagnostics: Vec<CompileError>,
    /// Lint findings, which do not fail the compilation.
    pub lint_findings: Vec<LintFinding>,
    /// Rule name to severity map in effect for the lint findings above.
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
