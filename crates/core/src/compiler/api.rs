use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_js_oxc::{ParsedJavaScript, parse, parse_with_type_facts};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFact};
use stillwind_source::{CompileError, SourceFile, collect_lint_directives};

use crate::{
    DesignSystemSnapshot, Fingerprint, ModuleAnalysis, ModuleCompiler, OxcSourceAnalyzer,
    SourceAnalyzer,
};

use super::Compiler;
use super::react_types::render_react_types;
use super::types::{CompileResult, CompilerOptions};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CompilerThemeError(#[from] pub crate::DesignSystemError);

impl Compiler {
    pub fn check_javascript(source: &SourceFile) -> Vec<CompileError> {
        parse(source).diagnostics
    }

    pub fn analyze_javascript(source: &SourceFile) -> ParsedJavaScript {
        parse(source)
    }

    /// Reads the project's CSS-first theme surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the stylesheet is invalid or contains an
    /// unterminated `@theme` block.
    pub fn read_theme(
        source: &SourceFile,
    ) -> Result<stillwind_css::ThemeIndex, CompilerThemeError> {
        DesignSystemSnapshot::from_source(source, CompilerOptions::default())
            .map(|snapshot| snapshot.theme().clone())
            .map_err(CompilerThemeError::from)
    }

    /// Generates a module augmentation containing the theme-backed utility vocabulary.
    ///
    /// # Errors
    ///
    /// Returns an error when the CSS theme cannot be parsed.
    pub fn generate_react_types(theme_source: &SourceFile) -> Result<String, CompilerThemeError> {
        let theme = Self::read_theme(theme_source)?;
        Ok(render_react_types(&theme))
    }

    /// Wraps compiled module CSS fragments in the project's shared stylesheet layers.
    ///
    /// # Errors
    ///
    /// Returns an error when the CSS theme cannot be parsed.
    pub fn render_react_stylesheet(
        theme_source: &SourceFile,
        module_css: &str,
        options: &CompilerOptions,
    ) -> Result<String, CompilerThemeError> {
        DesignSystemSnapshot::from_source(theme_source, options.clone())
            .map(|snapshot| snapshot.render_stylesheet(module_css))
            .map_err(CompilerThemeError::from)
    }

    /// Compiles the first static React subset into CSS.
    ///
    /// This temporary API deliberately supports only literal utility-string
    /// arguments. Conditions, attributes, selectors, and transforms are added
    /// in successive compiler slices.
    #[must_use]
    pub fn compile_react(source: &SourceFile, theme_source: &SourceFile) -> CompileResult {
        Self::compile_react_with_options(source, theme_source, &CompilerOptions::default())
    }

    #[must_use]
    pub fn compile_react_with_options(
        source: &SourceFile,
        theme_source: &SourceFile,
        options: &CompilerOptions,
    ) -> CompileResult {
        let scan = OxcSourceAnalyzer.scan(Arc::new(source.clone()));
        let parsed = OxcSourceAnalyzer.finalize(scan, None).parsed();
        Self::compile_react_parsed(source, theme_source, parsed, options)
    }

    #[must_use]
    pub fn compile_react_with_type_facts(
        source: &SourceFile,
        theme_source: &SourceFile,
        type_facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
    ) -> CompileResult {
        Self::compile_react_with_type_facts_and_options(
            source,
            theme_source,
            type_facts,
            assets,
            &CompilerOptions::default(),
        )
    }

    #[must_use]
    pub fn compile_react_with_type_facts_and_options(
        source: &SourceFile,
        theme_source: &SourceFile,
        type_facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
        options: &CompilerOptions,
    ) -> CompileResult {
        let parsed = parse_with_type_facts(source, type_facts, assets);
        Self::compile_react_parsed(source, theme_source, parsed, options)
    }

    #[allow(clippy::too_many_lines)] // Coordinates source transforms with CSS emission.
    fn compile_react_parsed(
        source: &SourceFile,
        theme_source: &SourceFile,
        parsed: ParsedJavaScript,
        options: &CompilerOptions,
    ) -> CompileResult {
        let snapshot = match DesignSystemSnapshot::from_source(theme_source, options.clone()) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return CompileResult {
                    diagnostics: vec![CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        theme_source.full_span(),
                    )],
                    ..CompileResult::default()
                };
            }
        };
        let analysis = ModuleAnalysis {
            external_assets: BTreeSet::new(),
            module_id: source.path().into(),
            source: Arc::new(source.clone()),
            source_revision: Fingerprint::of(source.text()),
            ir: parsed.module,
            summary: parsed.summary,
            lint_directives: collect_lint_directives(source.text()),
            diagnostics: parsed.diagnostics,
            lint_findings: parsed.lint_findings,
            semantic_revision: None,
            integration_diagnostics: Vec::new(),
            frontend_kind: "oxc".into(),
        };
        ModuleCompiler::new(snapshot.clone())
            .compile(&analysis)
            .to_legacy(&snapshot)
    }
}
