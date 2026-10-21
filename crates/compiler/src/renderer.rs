//! Stylesheet rendering entry points over one immutable snapshot.
//!
//! [`StylesheetRenderer`] assembles CSS from the design system's output template: it
//! expands authored directives, selects the theme tokens, keyframes, and `@property`
//! registrations the CSS actually needs, and walks the template's layers and slots.
//! [`StylesheetPlacement`] decides whether authored document rules and Preflight
//! participate, so a per-module carrier does not duplicate them. Assembly failures come
//! back as [`StylesheetRenderError`] with any naming fallback reports. Rendering a whole
//! compilation attaches the fatal cause to the source and invalidates generated output.

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::Span;

use crate::compiler::{
    detached_symbol, render_mapped, render_module_stylesheet, render_stylesheet,
};
use crate::identifiers::{IdentifierContext, Identifiers, RejectedIdentifier, RejectedNames};
use crate::{
    CompileResult, CompiledModule, CompilerOptions, GeneratedSymbol, MappedStylesheet, SymbolOrigin,
};

/// Where a rendered stylesheet is going to be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StylesheetPlacement {
    /// The single document stylesheet: authored rules and the support layer are included.
    Document,
    /// A module's own CSS carrier: only what that module needs, without document rules.
    Carrier,
}

/// A failed render and the generator rejections observed before it failed.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{diagnostic}")]
pub struct StylesheetRenderError {
    /// Fatal assembly or generated-name collision failure. No CSS is published.
    pub diagnostic: CompileDiagnostic,
    /// Development fallbacks observed during this render, in request order.
    pub rejected_names: Vec<RejectedIdentifier>,
}

impl From<CompileDiagnostic> for StylesheetRenderError {
    fn from(diagnostic: CompileDiagnostic) -> Self {
        Self {
            diagnostic,
            rejected_names: Vec::new(),
        }
    }
}

/// Assembled CSS and the generator rejections observed in one render.
#[derive(Clone, Debug)]
pub struct RenderedStylesheet {
    /// Complete CSS, source mappings, and template segments for this placement.
    pub stylesheet: MappedStylesheet,
    /// Development fallbacks, deduplicated within this render in request order.
    pub rejected_names: Vec<RejectedIdentifier>,
}

/// Renders stylesheets against a snapshot held for the renderer's lifetime.
pub struct StylesheetRenderer<'a> {
    /// Pinned design-system revision supplying the output template and registries.
    snapshot: &'a DesignSystemSnapshot,
    /// Compiler configuration controlling generated names in rendered output.
    options: CompilerOptions,
    /// One compilation's name allocation, shared by every stylesheet this renderer
    /// produces so a document and its carriers agree on the names they share.
    identifiers: IdentifierContext,
}

impl<'a> StylesheetRenderer<'a> {
    /// Creates a renderer for `snapshot`.
    ///
    /// The options' prefixes are resolved against the snapshot, so rendered text always
    /// agrees with the theme being rendered. `identifiers` names every generated helper
    /// and anonymous layer this renderer writes.
    pub fn new(
        snapshot: &'a DesignSystemSnapshot,
        options: &CompilerOptions,
        identifiers: &Identifiers,
    ) -> Self {
        Self {
            snapshot,
            options: options.clone().resolved(snapshot),
            identifiers: identifiers.context(options.hash_length),
        }
    }

    /// Produces standalone compilation output, including document CSS.
    /// Assembly failures become diagnostics and invalidate the generated output.
    ///
    /// On success `css` holds the assembled document stylesheet and `module_css`
    /// retains the unwrapped module rules. Assembly failure clears both output rules
    /// and source edits and records a diagnostic at `diagnostic_span`. That span must
    /// be valid in the caller's reporting source; it is used without coordinate
    /// validation.
    pub fn render_compilation(
        &self,
        module: &CompiledModule,
        diagnostic_span: Span,
    ) -> CompileResult {
        let mut result = module.to_module_output();
        result.css.clear();
        if module.is_success() {
            match self.render_mapped_stylesheet(&module.css.mapped(), StylesheetPlacement::Document)
            {
                Ok(css) => {
                    result.css = css.stylesheet.css;
                    result.rejected_names =
                        merge_rejected_names(result.rejected_names, css.rejected_names);
                }
                Err(error) => {
                    result.rejected_names =
                        merge_rejected_names(result.rejected_names, error.rejected_names);
                    result
                        .diagnostics
                        .push(CompileError::new(error.diagnostic, diagnostic_span));
                    result.module_css.clear();
                    result.contributions.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
        }
        // Helpers written for authored CSS and anonymous layers belong to no
        // declaration, so they are reported under the stylesheet that named them.
        let detached = self.take_detached_symbols();
        if result.diagnostics.is_empty() {
            result.symbols.extend(detached);
        }
        result
    }

    /// Takes the names this renderer produced that belong to no declaration.
    ///
    /// A renderer holds one compilation's name allocation, so this drains what its
    /// renders since the last call produced: conditional helpers written for authored
    /// CSS or theme fragments, and anonymous layers. They are reported under the
    /// stylesheet that named them and carry no declaration position, so a document-wide
    /// pool check can compare them against every other name the document uses.
    pub fn take_detached_symbols(&self) -> Vec<GeneratedSymbol> {
        let stylesheet = self
            .snapshot
            .theme()
            .output_template()
            .source_identity
            .clone();
        self.identifiers
            .drain_detached()
            .iter()
            .map(|name| detached_symbol(name, SymbolOrigin::Stylesheet(stylesheet.clone())))
            .collect()
    }

    /// Assembles document CSS for raw utility rules. Generated artifacts must use
    /// [`Self::render_mapped_stylesheet`] to retain `@keyframes` placement and explicit
    /// registration dependencies.
    ///
    /// # Errors
    ///
    /// Returns missing-slot, malformed property requirement or usage, and unresolved
    /// authored theme-reference errors. No partial stylesheet is returned.
    pub fn render_stylesheet(
        &self,
        module_css: &str,
    ) -> Result<RenderedStylesheet, StylesheetRenderError> {
        self.finish_render(
            render_stylesheet(
                self.snapshot.theme(),
                module_css,
                &self.options,
                &self.identifiers,
            )
            .map(|css| MappedStylesheet {
                css: (css),
                ..MappedStylesheet::default()
            }),
        )
    }

    /// Renders one producer's CSS and required tokens, keyframes and properties.
    /// The output is a carrier fragment: authored document rules and Preflight are
    /// omitted. Use [`Self::render_mapped_stylesheet`] when the caller needs to select
    /// document placement or preserve generated-rule provenance.
    ///
    /// # Errors
    ///
    /// Returns missing-slot, malformed property requirement or usage, and unresolved
    /// theme-reference errors in retained carrier CSS.
    pub fn render_module_stylesheet(
        &self,
        module_css: &str,
    ) -> Result<RenderedStylesheet, StylesheetRenderError> {
        self.finish_render(
            render_module_stylesheet(
                self.snapshot.theme(),
                module_css,
                &self.options,
                &self.identifiers,
            )
            .map(|css| MappedStylesheet {
                css: (css),
                ..MappedStylesheet::default()
            }),
        )
    }

    /// Renders a producer's CSS with source provenance attached.
    ///
    /// # Errors
    ///
    /// Returns malformed property requirements or usage CSS, unresolved authored theme
    /// references, or missing utility/support slots. The returned cause has no primary
    /// label; callers attach a location suitable for their document or carrier.
    pub fn render_mapped_stylesheet(
        &self,
        module_css: &MappedStylesheet,
        placement: StylesheetPlacement,
    ) -> Result<RenderedStylesheet, StylesheetRenderError> {
        self.finish_render(render_mapped(
            self.snapshot.theme().output_template(),
            self.snapshot.theme(),
            module_css,
            &self.options,
            placement == StylesheetPlacement::Document,
            Some(self.snapshot.sources()),
            &self.identifiers,
        ))
    }

    /// Takes this call's rejection records after either successful or failed assembly.
    fn finish_render(
        &self,
        result: Result<MappedStylesheet, CompileDiagnostic>,
    ) -> Result<RenderedStylesheet, StylesheetRenderError> {
        let rejected_names = self.identifiers.drain_rejected().into_entries();
        match result {
            Ok(stylesheet) => Ok(RenderedStylesheet {
                stylesheet,
                rejected_names,
            }),
            Err(diagnostic) => Err(StylesheetRenderError {
                diagnostic,
                rejected_names,
            }),
        }
    }
}

/// Combines generator reports in encounter order, deduplicating family, hash, and reason.
pub fn merge_rejected_names(
    first: Vec<RejectedIdentifier>,
    second: Vec<RejectedIdentifier>,
) -> Vec<RejectedIdentifier> {
    let mut rejected = RejectedNames::default();
    for entry in first.into_iter().chain(second) {
        rejected.push(entry);
    }
    rejected.into_entries()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use lean_string::LeanString;

    use stillwind_design_system::{DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions};
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, SourceRegistry};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{
        ClassContext, CompilerOptions, GeneratedName, IdentifierGenerator, IdentifierHash,
        IdentifierMode, Identifiers, LayerContext, LayerName, ModuleCompiler, StylesheetRenderer,
        UtilityClass,
    };

    #[derive(Debug, Default)]
    struct RejectNames {
        layers: Cell<usize>,
    }

    impl IdentifierGenerator for RejectNames {
        fn layer_name(
            &self,
            _context: &LayerContext,
            _hash: &IdentifierHash,
        ) -> Result<GeneratedName<LayerName>, LeanString> {
            self.layers.set(self.layers.get() + 1);
            Err("layer rejected".into())
        }

        fn utility_class(
            &self,
            _context: &ClassContext,
            _hash: &IdentifierHash,
        ) -> Result<GeneratedName<UtilityClass>, LeanString> {
            Err("class rejected".into())
        }
    }

    #[test]
    fn every_render_returns_fallback_reports_while_names_remain_memoized() {
        let design = snapshot("@layer { @stillwind-slot utilities; }");
        let options = CompilerOptions::default();
        let generator = Rc::new(RejectNames::default());
        let identifiers = Identifiers::new(Some(generator.clone()), IdentifierMode::Development);
        let renderer = StylesheetRenderer::new(&design, &options, &identifiers);
        let input = MappedStylesheet {
            css: ".x{display:flex}".into(),
            ..MappedStylesheet::default()
        };
        let first = renderer
            .render_mapped_stylesheet(&input, StylesheetPlacement::Document)
            .unwrap();
        assert_eq!(first.rejected_names.len(), 1);
        assert_eq!(first.rejected_names[0].reason, "layer rejected");
        assert!(first.rejected_names[0].substituted);
        for next in [
            renderer
                .render_mapped_stylesheet(&input, StylesheetPlacement::Carrier)
                .unwrap(),
            renderer.render_stylesheet(&input.css).unwrap(),
            renderer.render_module_stylesheet(&input.css).unwrap(),
        ] {
            assert_eq!(next.rejected_names, first.rejected_names);
            assert_eq!(
                next.stylesheet.css.rsplit("@layer sw-anon-").next(),
                first.stylesheet.css.rsplit("@layer sw-anon-").next()
            );
        }
        assert_eq!(generator.layers.get(), 1);
        let release = Identifiers::new(Some(generator), IdentifierMode::Release);
        let error = StylesheetRenderer::new(&design, &options, &release)
            .render_stylesheet(&input.css)
            .unwrap_err();
        assert!(matches!(
            error.diagnostic,
            CompileDiagnostic::IdentifierGenerationFailed { .. }
        ));
        assert_eq!(error.rejected_names, [] as [RejectedIdentifier; 0]);
    }

    #[test]
    fn caller_constructed_rules_validate_registration_dependencies_during_rendering() {
        let design = snapshot("@stillwind-slot utilities;");
        let syntax = stillwind_css::CssRule::parse(".x{display:flex}")
            .unwrap()
            .remove(0);
        let artifact = crate::ModuleCssArtifact {
            rules: vec![crate::GeneratedRule::new(
                syntax,
                false,
                vec!["--unknown-registration".into()],
            )],
        };
        let options = CompilerOptions::default();
        let identifiers = Identifiers::default();
        let renderer = StylesheetRenderer::new(&design, &options, &identifiers);
        for placement in [StylesheetPlacement::Document, StylesheetPlacement::Carrier] {
            let error = renderer
                .render_mapped_stylesheet(&artifact.mapped(), placement)
                .unwrap_err();
            assert_eq!(
                error.diagnostic,
                CompileDiagnostic::UnknownRequiredProperty {
                    name: "--unknown-registration".into()
                }
            );
        }
    }

    fn snapshot(css: &str) -> DesignSystemSnapshot {
        DesignSystemSnapshot::from_source(
            &css_source(FileId(90), css).unwrap(),
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap()
    }

    /// Rendering a compilation keeps the module fragment separate from the document CSS.
    #[test]
    fn document_rendering_keeps_the_module_fragment() {
        let design =
            snapshot("@utility flex { display:flex; } @layer { @stillwind-slot utilities; }");
        let source = Arc::new(
            SourceFile::new(
                FileId(1),
                None,
                None,
                "import { style } from 'stillwind/react'; export const x = style('flex');",
            )
            .unwrap(),
        );
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source.clone(),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
        let options = CompilerOptions::default();
        let identifiers = Identifiers::new(
            Some(Rc::new(RejectNames::default())),
            IdentifierMode::Development,
        );
        let compiled =
            ModuleCompiler::new(design.clone(), options.clone()).compile(&analysis, &identifiers);
        assert!(compiled.is_success());
        let output = StylesheetRenderer::new(&design, &options, &identifiers)
            .render_compilation(&compiled, source.full_span());
        assert_eq!(output.diagnostics, [] as [CompileError; 0]);
        assert_ne!(output.module_css, "");
        assert_eq!(output.module_css, compiled.css.to_css());
        assert_eq!(
            output
                .rejected_names
                .iter()
                .map(|entry| entry.reason.as_str())
                .collect::<Vec<_>>(),
            ["class rejected", "layer rejected"]
        );
    }

    /// Authored CSS reached through an import keeps the physical file and byte range it
    /// came from, and the same stylesheet renders identically with and without mappings.
    #[test]
    fn imported_authored_css_and_theme_tokens_keep_their_physical_sources() {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        let entry = "@import 'child.css' layer(parts) supports(display:grid) screen;\n\
                     @theme { --color-brand: red; } :root,:host { @stillwind-slot theme; }";
        let DesignSystemLoadStep::NeedFile(requests) = loader.provide(
            0,
            CssSourceName::from("entry.css"),
            None,
            None,
            entry.into(),
        ) else {
            panic!("import")
        };
        let child = "/* 日本語 🧭 */\n.imported { color:var(--color-brand); }";
        let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
            requests[0].id,
            CssSourceName::from("child.css"),
            None,
            None,
            child.into(),
        ) else {
            panic!("complete")
        };
        let output = StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default(),
        )
        .render_mapped_stylesheet(&MappedStylesheet::default(), StylesheetPlacement::Document)
        .unwrap()
        .stylesheet;
        assert_eq!(
            output.css,
            StylesheetRenderer::new(
                &snapshot,
                &CompilerOptions::default(),
                &Identifiers::default()
            )
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
        );
        let authored = output
            .mappings
            .iter()
            .find(|mapping| mapping.source.text() == child)
            .unwrap();
        assert!(authored.exact);
        assert_eq!(
            &output.css[authored.generated.clone()],
            &child[authored.original.clone()]
        );
        assert!(
            output
                .mappings
                .iter()
                .any(|mapping| mapping.source.text() == entry
                    && output.css[mapping.generated.clone()].contains("--color-brand"))
        );
        assert!(output.css.contains("@supports (display:grid)"));
    }
}
