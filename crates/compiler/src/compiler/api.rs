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

#[derive(Debug, thiserror::Error)]
pub enum CompilerThemeError {
    #[error("{0}")]
    DesignSystem(#[from] DesignSystemError),
    #[error("{0}")]
    Stylesheet(#[from] StylesheetError),
}

impl Compiler {
    /// # Errors
    /// Returns invalid CSS or unresolved imports.
    pub fn read_theme(source: &SourceFile) -> Result<ThemeIndex, CompilerThemeError> {
        Ok(
            DesignSystemSnapshot::from_source(source, DesignSystemOptions::default())?
                .theme()
                .clone(),
        )
    }

    /// # Errors
    /// Returns invalid CSS or unresolved imports.
    pub fn generate_react_types(source: &SourceFile) -> Result<String, CompilerThemeError> {
        Ok(render_react_types(&Self::read_theme(source)?))
    }

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

    pub fn compile_source(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
    ) -> CompileResult {
        Self::compile_source_with_options(analyzer, source, theme, &CompilerOptions::default())
    }

    pub fn compile_source_with_options(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        theme: &SourceFile,
        options: &CompilerOptions,
    ) -> CompileResult {
        let scan = analyzer.scan(Arc::new(source.clone()));
        Self::compile_analysis(&analyzer.finalize(scan, None), theme, options)
    }

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
        ModuleCompiler::new(snapshot.clone(), options.clone())
            .compile(analysis)
            .to_output(&snapshot, options)
    }
}
