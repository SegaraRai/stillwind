//! The compilation result shape handed to a host.
//!
//! [`CompileResult`] contains diagnostics, source edits, generated identifiers, and CSS.
//! Its [`CssContribution`] entries carry the placement and registration dependencies
//! needed by artifact construction. `module_css` is a text-only inspection view;
//! `css` holds the output selected by the caller's rendering stage. Serde excludes
//! contributions and module text because host responses transport the rendered CSS.
//!
//! [`CompileResult::apply_to_source`] applies the stored edits to their original source
//! and rejects failed compilations. This module defines transport values; compilation,
//! slot assembly, and host resource resolution belong to their respective callers.

use serde::Serialize;

use stillwind_diagnostics::{CompileError, LintFinding, LintSettings};
use stillwind_ir::StyleDependency;
use stillwind_source::SourceFile;

use super::props::GeneratedClass;
use super::symbols::GeneratedSymbol;
use crate::{
    ModuleResourceReference, RejectedIdentifier,
    transforms::{SourceTransform, TransformError, apply_transforms},
};

/// Everything one compilation produced.
///
/// Successful pipeline output contains CSS, generated identifiers and source edits.
/// Marker classes and asset symbols need not own a CSS rule. Failed pipeline output
/// clears generated output; public construction does not enforce that relationship.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    /// Generated contributions in emission order, including placement and registration
    /// dependencies required for artifact construction.
    #[serde(skip)]
    pub contributions: Vec<CssContribution>,
    /// Style dependencies a consumer must order this module after.
    pub composition: Vec<StyleDependency>,
    /// Host-resolvable URLs that were replaced by placeholders in the CSS.
    pub resources: Vec<ModuleResourceReference>,
    /// CSS output: module text in a producer-only result, or the assembled stylesheet
    /// after rendering. Check diagnostics before consuming it.
    pub css: String,
    /// Concatenated unwrapped CSS for inspection. Placement and explicit registration
    /// dependencies are available only in `contributions`, not in this string.
    #[serde(skip)]
    pub module_css: String,
    /// Classes generated at this module's callsites, with their guard paths.
    pub classes: Vec<GeneratedClass>,
    /// Generated CSS identifiers with the identities they were allocated from.
    pub symbols: Vec<GeneratedSymbol>,
    /// Names a host generator rejected, which the host may warn about.
    ///
    /// These are not lint findings: a rejected name comes from the generator rather than
    /// from the source, so it carries no source location. A development compilation
    /// substitutes the built-in name and records the rejection here; a release
    /// compilation fails with a diagnostic instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected_names: Vec<RejectedIdentifier>,
    /// Replacements to apply to the original source.
    pub transforms: Vec<SourceTransform>,
    /// Errors that failed the compilation.
    pub diagnostics: Vec<CompileError>,
    /// Lint findings, which do not fail the compilation.
    pub lint_findings: Vec<LintFinding>,
    /// Settings used to report the lint findings above; serialized as rule names to severities.
    pub lint_settings: LintSettings,
}

impl CompileResult {
    /// Validates and applies all stored source replacements in source order.
    ///
    /// The replacements include macro calls and import/runtime edits. `source` must
    /// be the original buffer addressed by their spans; the returned text contains
    /// the complete edit result, while the input buffer and stored edits are unchanged.
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

/// CSS emitted by one class unit or motion definition, with its assembly metadata.
///
/// Artifact construction splits `css` into owned rules; each rule inherits the
/// contribution's destination and registration requirements. Serializing only `css`
/// loses that metadata and is insufficient for reconstructing the generated artifact.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssContribution {
    /// Complete generated rule blocks in emission order.
    pub css: String,
    /// True when all blocks are generated animation definitions for the keyframes
    /// slot; false when they belong to the utilities contribution.
    pub keyframes: bool,
    /// Custom-property names requiring registration. Encounter order and duplicates
    /// are allowed here; artifact construction sorts and deduplicates them.
    pub required_properties: Vec<String>,
}
