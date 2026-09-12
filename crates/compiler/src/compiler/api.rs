//! Stateless compilation entry points.
//!
//! [`Compiler`] has no instances: each function takes the analyzer, sources, and options, and
//! returns a [`CompileResult`]. A theme that cannot be read becomes a `SHCSS001` diagnostic
//! against the theme file instead of an error, so a caller always receives a result shape it
//! knows how to report; only the theme-reading helpers return a [`CompilerThemeError`].
//!
//! The type-fact variants exist for hosts that already ran a TypeScript checker: the facts
//! and asset metadata are folded into a versioned [`TypeFacts`] value whose revision the
//! frontend's finalize step consumes, so a compilation is reproducible from the recorded
//! inputs alone.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_design_system::{
    DesignSystemError, DesignSystemOptions, DesignSystemSnapshot, ThemeIndex,
};
use stillwind_ir::{ModuleAnalysis, SourceAnalyzer};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFact, TypeFacts};
use stillwind_source::{CompileError, Fingerprint, SourceFile};

use super::react::declarations::render_react_types;
use super::{CompileResult, Compiler};
use crate::{CompilerOptions, ModuleCompiler, StylesheetError, StylesheetRenderer};

/// A failure to read or render the theme file.
#[derive(Debug, thiserror::Error)]
pub enum CompilerThemeError {
    /// The theme CSS could not be turned into a design system.
    #[error("{0}")]
    DesignSystem(#[from] DesignSystemError),
    /// The theme CSS parsed, but a stylesheet could not be assembled around it.
    #[error("{0}")]
    Stylesheet(#[from] StylesheetError),
}

impl Compiler {
    /// Parses a theme buffer into its index, without keeping the snapshot.
    ///
    /// # Errors
    /// Returns invalid CSS or unresolved imports.
    pub fn read_theme(source: &SourceFile) -> Result<ThemeIndex, CompilerThemeError> {
        Ok(
            DesignSystemSnapshot::from_source(source, DesignSystemOptions::default())?
                .theme()
                .clone(),
        )
    }

    /// Renders the ambient type declarations for a theme buffer.
    ///
    /// # Errors
    /// Returns invalid CSS or unresolved imports.
    pub fn generate_react_types(source: &SourceFile) -> Result<String, CompilerThemeError> {
        Ok(render_react_types(&Self::read_theme(source)?))
    }

    /// Renders a standalone stylesheet for a theme buffer and utility rules.
    ///
    /// # Errors
    /// Returns invalid CSS or unresolved imports.
    pub fn render_react_stylesheet(
        source: &SourceFile,
        module_css: &str,
        options: &CompilerOptions,
    ) -> Result<String, CompilerThemeError> {
        let snapshot = DesignSystemSnapshot::from_source(source, options.design_system())?;
        Ok(StylesheetRenderer::new(&snapshot, options).render_stylesheet(module_css)?)
    }

    /// Scans, finalizes, and compiles one source with default options and no type facts.
    pub fn compile_source(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
    ) -> CompileResult {
        Self::compile_source_with_options(analyzer, source, theme, &CompilerOptions::default())
    }

    /// Compiles one source against an explicit theme buffer and options.
    pub fn compile_source_with_options(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
        options: &CompilerOptions,
    ) -> CompileResult {
        let scan = analyzer.scan(Arc::new(source.clone()));
        Self::compile_analysis(&analyzer.finalize(scan, None), theme, options)
    }

    /// Compiles one source with exported type facts and asset metadata.
    ///
    /// The facts are recorded together with the scan's module and source revisions, so a
    /// caller that replays the analysis cannot pair facts with the wrong revision.
    pub fn compile_source_with_type_facts(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
        facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
    ) -> CompileResult {
        Self::compile_source_with_type_facts_and_options(
            analyzer,
            source,
            theme,
            facts,
            assets,
            &CompilerOptions::default(),
        )
    }

    /// The options-taking variant of [`Compiler::compile_source_with_type_facts`].
    pub fn compile_source_with_type_facts_and_options(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
        facts: &[TypeFact],
        assets: &BTreeMap<IdentityId, AssetMetadata>,
        options: &CompilerOptions,
    ) -> CompileResult {
        let scan = analyzer.scan(Arc::new(source.clone()));
        let facts = TypeFacts {
            schema: TypeFacts::SCHEMA,
            module_id: scan.module_id.clone(),
            source_revision: scan.source_revision.clone(),
            semantic_revision: Fingerprint::of(format!("{facts:?}{assets:?}")),
            facts: facts.to_vec(),
            assets: assets.clone(),
        };
        Self::compile_analysis(&analyzer.finalize(scan, Some(&facts)), theme, options)
    }

    /// Compiles an already finalized analysis against a theme buffer.
    ///
    /// A design-system failure is reported as a `SHCSS001` diagnostic spanning the whole
    /// theme file, with an empty result otherwise, because there is no module output to
    /// return once the theme is unusable.
    pub fn compile_analysis(
        analysis: &ModuleAnalysis,
        theme: &SourceFile,
        options: &CompilerOptions,
    ) -> CompileResult {
        let snapshot = match DesignSystemSnapshot::from_source(theme, options.design_system()) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return CompileResult {
                    diagnostics: vec![CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        theme.full_span(),
                    )],
                    ..CompileResult::default()
                };
            }
        };
        let module = ModuleCompiler::new(snapshot.clone(), options.clone()).compile(analysis);
        crate::StylesheetRenderer::new(&snapshot, options)
            .render_compilation(&module, analysis.source.full_span())
    }
}
