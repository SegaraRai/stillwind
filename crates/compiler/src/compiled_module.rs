//! Stores compiled output and the keys used to reuse it.
//!
//! [`CompiledModule::apply_to_source`] checks source edits before applying them.
//! [`CompiledModule::to_module_output`] returns a producer's CSS without assembling
//! the document stylesheet. [`super::ModuleCompiler`] performs compilation.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Serialize;

use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::StyleDependency;
use stillwind_semantic::IdentityId;
use stillwind_source::{Fingerprint, ModuleId, SourceFile};

use crate::transforms::apply_transforms;
use crate::{
    CompileResult, CssContribution, GeneratedClass, GeneratedSymbol, ModuleCssArtifact,
    RejectedIdentifier, SourceTransform, StyleEffects, TransformError,
};

/// A source-transform failure as reported by [`CompiledModule::apply_to_source`].
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ModuleTransformError(
    /// Underlying validation or application failure for the stored source edits.
    #[from]
    pub TransformError,
);

/// Identifies the inputs used to compile a module so its output can be reused.
///
/// Equal keys allow reuse of the compiled artifact. [`Self::compiler_version`] includes
/// the compiler's build fingerprint, so a compiler rebuild invalidates cached artifacts.
/// [`Self::analysis_revision`] hashes the analysis inputs supplied by the frontend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleArtifactKey {
    /// Compiler version plus the build fingerprint of the compiler and its dependencies.
    pub compiler_version: String,
    /// Hash of the resolved [`crate::CompilerOptions`].
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

/// Records definitions that can affect a compiled module's output.
///
/// Sets may contain unused names, so a definition change may invalidate output that is
/// still valid. They must include every dependency to avoid keeping stale CSS.
/// [`Self::assets`] contains the identities of referenced refs, keyframes, and view
/// transitions.
///
/// Utility, variant, and property names use `Arc` to share the snapshot's sets. Each
/// module can keep these sets without copying all their names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationDependencies {
    /// Candidate spellings the module mentions, including their variant prefixes.
    pub candidates: BTreeSet<String>,
    /// Theme token names whose values appear in the module's generated CSS.
    pub theme_tokens: BTreeSet<String>,
    /// All registered utility definition names, including unused definitions.
    pub utilities: Arc<BTreeSet<String>>,
    /// All registered variant definition names, including unused definitions.
    pub variants: Arc<BTreeSet<String>>,
    /// All currently registered custom-property definition names.
    pub properties: Arc<BTreeSet<String>>,
    /// Identities of the assets the module used.
    pub assets: BTreeSet<IdentityId>,
}

/// A URL in generated CSS that only the host can resolve.
///
/// The compiler replaces the URL with a placeholder, and the host resolves it later.
/// References with the same owner and decoded specifier share a placeholder. The
/// placeholder hash includes the owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
/// The compiler returns CSS, classes, symbols, and source edits together. Marker
/// classes may have no CSS rule. Callers must check for compilation errors before using
/// output. Applying source edits also checks that their spans are valid.
#[derive(Clone, Debug)]
pub struct CompiledModule {
    /// Effective lint settings captured during compilation, including project conversion options.
    pub lint_settings: stillwind_diagnostics::LintSettings,
    /// Records the effects of this module's own style calls and keeps their source
    /// coordinates. The vector is empty after compilation failure. Hosts use
    /// [`crate::StyleResolver`] to update composition findings when bases change.
    pub style_effects: Vec<StyleEffects>,
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
    /// Records definitions whose changes invalidate this artifact.
    pub dependencies: CompilationDependencies,
}

impl CompiledModule {
    /// Applies the module's transforms to the original source and returns the new text.
    ///
    /// Leaves `source` unchanged and returns a new string. Text outside the edit spans
    /// is preserved byte for byte.
    ///
    /// # Errors
    ///
    /// Returns an error if compilation failed or source edit spans are invalid or
    /// overlap.
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
    /// The returned `css` and `module_css` both contain this module's rules and keep
    /// the lint policy captured during compilation. The stylesheet renderer later
    /// replaces `css` with the document stylesheet and keeps `module_css` for hosts
    /// that send CSS per module.
    ///
    /// The result copies style effects and existing findings without resolving bases
    /// again. Callers must update composition findings when the graph changes.
    pub fn to_module_output(&self) -> CompileResult {
        let module_css = self.css.to_css();
        CompileResult {
            style_effects: self.style_effects.clone(),
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
            lint_settings: self.lint_settings.clone(),
        }
    }
}
