//! Stateless compilation entry points.
//!
//! Each [`Compiler`] compilation function takes its analyzer or analysis, sources, and
//! options, and returns a [`CompileResult`]. A theme that cannot be read becomes a CSS
//! diagnostic against the theme file instead of an error, so a caller always receives a
//! result shape it knows how to report. [`Compiler::render_stylesheet`] returns a
//! [`CompileError`] instead.
//!
//! Hosts that already ran a TypeScript checker finalize the analysis with their
//! versioned type facts and pass it to [`Compiler::compile_analysis`].

use std::sync::Arc;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::CompileError;
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_source::{CssSourceName, ModuleId, SourceFile};

use super::CompileResult;
use crate::{CompilerOptions, Identifiers, ModuleCompiler, StylesheetRenderer};

/// Stateless entry points for source compilation and stylesheet rendering.
pub struct Compiler;

impl Compiler {
    /// Renders a standalone stylesheet for a theme buffer and utility rules.
    ///
    /// Assembly failures are located at the whole theme buffer. The input buffers are
    /// unchanged; no partial stylesheet is returned on failure.
    ///
    /// # Errors
    ///
    /// Returns invalid CSS, unresolved imports, or invalid host configuration.
    /// Also rejects missing output slots, malformed property requirements or usage CSS,
    /// and unresolved authored theme references.
    pub fn render_stylesheet(
        source: &SourceFile,
        source_name: &CssSourceName,
        module_css: &str,
        options: &CompilerOptions,
    ) -> Result<String, CompileError> {
        let snapshot =
            DesignSystemSnapshot::from_source(source, source_name.clone(), options.design_system())
                .map_err(|error| error.into_compile_error(source.full_span()))?;
        StylesheetRenderer::new(&snapshot, options, &Identifiers::default())
            .render_stylesheet(module_css)
            .map(|output| output.stylesheet.css)
            .map_err(|error| CompileError::new(error.diagnostic, source.full_span()))
    }

    /// Compiles one source against an explicit theme buffer and options.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_source(
        analyzer: &impl SourceAnalyzer,
        source: &SourceFile,
        module_id: &ModuleId,
        language: ScriptLanguage,
        module_kind: ScriptModuleKind,
        theme: &SourceFile,
        theme_name: &CssSourceName,
        options: &CompilerOptions,
    ) -> CompileResult {
        let scan = analyzer.scan(
            Arc::new(source.clone()),
            module_id.clone(),
            language,
            module_kind,
        );
        Self::compile_analysis(&analyzer.finalize(scan, None), theme, theme_name, options)
    }

    /// Compiles an already finalized analysis against a theme buffer.
    ///
    /// A design-system failure is reported as a CSS diagnostic spanning the whole
    /// theme file, with an empty result otherwise, because there is no module output to
    /// return once the theme is unusable.
    pub fn compile_analysis(
        analysis: &ModuleAnalysis,
        theme: &SourceFile,
        theme_name: &CssSourceName,
        options: &CompilerOptions,
    ) -> CompileResult {
        let snapshot = match DesignSystemSnapshot::from_source(
            theme,
            theme_name.clone(),
            options.design_system(),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return CompileResult {
                    diagnostics: vec![error.into_compile_error(theme.full_span())],
                    ..CompileResult::default()
                };
            }
        };
        let module = ModuleCompiler::new(snapshot.clone(), options.clone())
            .compile(analysis, &Identifiers::default());
        StylesheetRenderer::new(&snapshot, options, &Identifiers::default())
            .render_compilation(&module, analysis.source.full_span())
    }
}
