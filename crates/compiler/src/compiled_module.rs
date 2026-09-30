//! Compiled module results, cache identity, and checked source-edit application.
//!
//! [`CompiledModule::apply_to_source`] validates edits before applying them;
//! [`CompiledModule::to_module_output`] exposes producer CSS without document assembly.
//! Compilation itself belongs to [`super::ModuleCompiler`].

use std::collections::BTreeSet;

use serde::Serialize;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::StyleDependency;
use stillwind_semantic::IdentityId;
use stillwind_source::{Fingerprint, ModuleId, SourceFile};

use crate::transforms::apply_transforms;
use crate::{
    CompileResult, CssContribution, GeneratedClass, GeneratedSymbol, ModuleCssArtifact,
    RejectedIdentifier, SourceTransform, TransformError,
};

/// A source-transform failure as reported by [`CompiledModule::apply_to_source`].
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ModuleTransformError(
    /// Underlying validation or application failure for the stored source edits.
    #[from]
    pub TransformError,
);

/// Everything that can change a module's compiled output, and nothing else.
///
/// Two equal keys mean the previously compiled artifact can be reused. The compiler's
/// own build fingerprint participates through [`Self::compiler_version`], so rebuilding
/// the compiler invalidates every cached artifact, and `analysis_revision` covers the
/// frontend's actual inputs rather than its implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleArtifactKey {
    /// Compiler version plus the build fingerprint of the compiler and its dependencies.
    pub compiler_version: String,
    /// Fingerprint of the resolved [`crate::CompilerOptions`]; a prefix change is a separate key.
    pub compiler_options: Fingerprint,
    /// The frontend's actual compiler inputs, independent of its implementation.
    pub analysis_revision: Fingerprint,
    /// Content fingerprint of the design system the module was compiled against.
    pub design_system: Fingerprint,
    /// Fingerprint of the source buffer, so an edit invalidates the artifact.
    pub source_revision: Fingerprint,
    /// Identity of the module inside the project graph.
    pub module_id: ModuleId,
    /// Identity of the source file, which may differ from the module id after projection.
    pub source_file_id: u32,
    /// Which frontend produced the analysis, for example `oxc`.
    pub frontend_kind: String,
    /// Adapter mode the analysis was extracted under.
    pub adapter_mode: String,
    /// Fingerprint of the type facts and asset metadata that shaped the analysis, when any.
    pub semantic_revision: Option<Fingerprint>,
}

/// Definition sets a compiled module's output was derived from.
///
/// The sets are conservative: they may hold more names than the output actually uses,
/// so a change can invalidate a still-valid artifact. They must never hold fewer,
/// because an omission would keep stale CSS. [`Self::assets`] lists the ref, keyframes,
/// and view-transition identities the module referenced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationDependencies {
    /// Candidate spellings the module mentions, including their variant prefixes.
    pub candidates: BTreeSet<String>,
    /// Theme token names whose values appear in the module's generated CSS.
    pub theme_tokens: BTreeSet<String>,
    /// All currently registered utility definition names, recorded conservatively.
    pub utilities: BTreeSet<String>,
    /// All currently registered variant definition names, recorded conservatively.
    pub variants: BTreeSet<String>,
    /// All currently registered custom-property definition names.
    pub properties: BTreeSet<String>,
    /// Identities of the assets the module used.
    pub assets: BTreeSet<IdentityId>,
}

/// A URL in generated CSS that only the host can resolve.
///
/// The compiler cannot know where a file lives, so each reference becomes a unique
/// placeholder string and the host rewrites it later. Repeated references with the
/// same owner and decoded specifier share a placeholder; different owners are hashed
/// separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModuleResourceReference {
    /// Token standing in for the URL in the emitted CSS, quoted or inside `url(...)`.
    pub placeholder: String,
    /// Module whose CSS contained the reference.
    pub owner: ModuleId,
    /// Decoded URL specifier, without the surrounding CSS syntax.
    pub specifier: String,
}

/// One module's complete compiled output.
///
/// This is the compiler-to-project boundary. The fields are produced together, so a
/// consumer receives generated classes and symbols alongside CSS and source edits.
/// Marker classes need not have a CSS rule. Transform validity is checked when the
/// edits are rendered. They are only meaningful when the module compiled without
/// diagnostics.
#[derive(Clone, Debug)]
pub struct CompiledModule {
    /// Style dependencies the project layer must order this module after.
    pub composition: Vec<StyleDependency>,
    /// URLs found in `css`, replaced there by their placeholders.
    pub resources: Vec<ModuleResourceReference>,
    /// Cache identity of this artifact.
    pub key: ModuleArtifactKey,
    /// Replacements for the original source; empty after a failed compilation.
    pub code_transforms: Vec<SourceTransform>,
    /// The rules this module owns; empty after a failed compilation.
    pub css: ModuleCssArtifact,
    /// Classes generated for this module's callsites.
    pub classes: Vec<GeneratedClass>,
    /// Generated CSS identifiers with the identities they were allocated from.
    pub symbols: Vec<GeneratedSymbol>,
    /// Names a host generator rejected, which the host may warn about.
    pub rejected_names: Vec<RejectedIdentifier>,
    /// Errors that failed the compilation, if any.
    pub diagnostics: Vec<CompileError>,
    /// Lint findings, which do not fail the compilation.
    pub lint_findings: Vec<LintFinding>,
    /// Definitions a change to which invalidates this artifact.
    pub dependencies: CompilationDependencies,
}

impl CompiledModule {
    /// Applies the module's transforms to the original source and returns the new text.
    ///
    /// The input source is not modified; the result is a fresh string with untouched
    /// intervals preserved byte for byte.
    ///
    /// # Errors
    ///
    /// Rejects failed compilation and invalid or overlapping source-transform spans.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, ModuleTransformError> {
        if !self.is_success() {
            return Err(ModuleTransformError(TransformError::CompilationFailed));
        }
        apply_transforms(source, &self.code_transforms).map_err(ModuleTransformError::from)
    }

    /// True when the module compiled without diagnostics.
    ///
    /// Transforms, classes, symbols, and CSS are empty whenever this is false, so a
    /// caller can use it as the single gate before applying output.
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Builds a producer's output without assembling document CSS or support layers.
    ///
    /// `css` and `module_css` are both the module's own rules here: the stylesheet
    /// renderer later replaces `css` with the assembled document stylesheet while
    /// leaving `module_css` addressable for a host that transports CSS per module.
    pub fn to_module_output(&self, snapshot: &DesignSystemSnapshot) -> CompileResult {
        let module_css = self.css.to_css();
        CompileResult {
            contributions: self
                .css
                .rules
                .iter()
                .map(|rule| CssContribution {
                    css: rule.to_css(),
                    keyframes: rule.keyframes,
                    required_properties: rule.required_properties.clone(),
                })
                .collect(),
            composition: self.composition.clone(),
            resources: self.resources.clone(),
            css: module_css.clone(),
            module_css,
            classes: self.classes.clone(),
            symbols: self.symbols.clone(),
            rejected_names: self.rejected_names.clone(),
            transforms: self.code_transforms.clone(),
            diagnostics: self.diagnostics.clone(),
            lint_findings: self.lint_findings.clone(),
            lint_settings: snapshot.theme().config.lint_settings.clone(),
        }
    }
}
