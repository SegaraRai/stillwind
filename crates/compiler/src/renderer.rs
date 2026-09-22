//! Stylesheet rendering entry points over one immutable snapshot.
//!
//! [`StylesheetRenderer`] assembles CSS from the design system's output template: it
//! expands authored directives, selects the theme tokens, keyframes, and `@property`
//! registrations the CSS actually needs, and walks the template's layers and slots.
//! [`StylesheetPlacement`] decides whether authored document rules and Preflight
//! participate, so a per-module carrier does not duplicate them. Assembly failures come
//! back as [`CompileErrorKind`], or, when rendering a whole compilation, as a `SHCSS001`
//! diagnostic that invalidates the generated output instead of shipping CSS known to be
//! incomplete.

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_source::Span;

use crate::compiler::{
    detached_symbol, render_mapped, render_module_stylesheet, render_stylesheet,
    render_vocabulary_types,
};
use crate::identifiers::{IdentifierContext, Identifiers};
use crate::{CompileResult, CompiledModule, CompilerOptions, GeneratedSymbol, MappedStylesheet};

/// Where a rendered stylesheet is going to be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StylesheetPlacement {
    /// The single document stylesheet: authored rules and the support layer are included.
    Document,
    /// A module's own CSS carrier: only what that module needs, without document rules.
    Carrier,
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

    /// Ambient TypeScript declarations for the framework vocabulary of this theme.
    pub fn generate_vocabulary_types(&self) -> String {
        render_vocabulary_types(self.snapshot.theme())
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
        let mut result = module.to_module_output(self.snapshot);
        result.css.clear();
        if module.is_success() {
            match self.render_mapped_stylesheet(&module.css.mapped(), StylesheetPlacement::Document)
            {
                Ok(css) => result.css = css.css,
                Err(error) => {
                    result
                        .diagnostics
                        .push(CompileError::new(error, diagnostic_span));
                    result.module_css.clear();
                    result.contributions.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
        }
        // TODO: this is the only entry point that drains the rejections. A host that
        // renders a carrier or a project stylesheet with its own generator never learns
        // which of its names were rejected; return them from those entry points too.
        result.rejected_names = self.identifiers.drain_rejected().into_entries();
        // Helpers written for authored CSS and anonymous layers belong to no
        // declaration, so they are reported under the stylesheet that named them.
        result.symbols.extend(self.take_detached_symbols());
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
            .output_template
            .source_identity
            .clone();
        self.identifiers
            .drain_detached()
            .iter()
            .map(|name| detached_symbol(name, &stylesheet))
            .collect()
    }

    /// Assembles document CSS for raw utility rules.
    /// Generated artifacts must use [`Self::render_mapped_stylesheet`] to retain
    /// keyframe placement and explicit registration dependencies.
    ///
    /// # Errors
    ///
    /// Returns missing-slot, malformed property requirement or usage, and unresolved
    /// authored theme-reference errors. No partial stylesheet is returned.
    pub fn render_stylesheet(&self, module_css: &str) -> Result<String, CompileErrorKind> {
        render_stylesheet(
            self.snapshot.theme(),
            module_css,
            &self.options,
            &self.identifiers,
        )
    }

    /// Document CSS with the layer reservations authored by the entry stylesheet.
    ///
    /// The default package reserves support order before any late producer; a custom
    /// entry owns its own layer order. Rendering never adds an implicit reservation.
    ///
    /// # Errors
    ///
    /// Returns an error when the authored CSS cannot be analyzed.
    pub fn render_base_stylesheet(&self) -> Result<String, CompileErrorKind> {
        self.render_stylesheet("")
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
    pub fn render_module_stylesheet(&self, module_css: &str) -> Result<String, CompileErrorKind> {
        render_module_stylesheet(
            self.snapshot.theme(),
            module_css,
            &self.options,
            &self.identifiers,
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
    ) -> Result<MappedStylesheet, CompileErrorKind> {
        render_mapped(
            &self.snapshot.theme().output_template,
            self.snapshot.theme(),
            module_css,
            &self.options,
            placement == StylesheetPlacement::Document,
            Some(self.snapshot.sources()),
            &self.identifiers,
        )
    }

    /// Document CSS with source provenance and CSS-authored layer reservations.
    ///
    /// This is the mapped counterpart of [`Self::render_base_stylesheet`] and keeps the
    /// same source order as the entry template.
    ///
    /// # Errors
    ///
    /// Returns invalid authored CSS.
    pub fn render_base_stylesheet_with_map(&self) -> Result<MappedStylesheet, CompileErrorKind> {
        self.render_mapped_stylesheet(&MappedStylesheet::default(), StylesheetPlacement::Document)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_design_system::DesignSystemOptions;
    use stillwind_ir::SourceAnalyzer;
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::{CompilerOptions, Identifiers, ModuleCompiler};

    fn snapshot(css: &str) -> DesignSystemSnapshot {
        DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(FileId(90), "theme.css", css).unwrap(),
            DesignSystemOptions::default(),
        )
        .unwrap()
    }

    /// Rendering a compilation keeps the module fragment separate from the document CSS.
    #[test]
    fn document_rendering_keeps_the_module_fragment() {
        let design = snapshot("@utility flex { display:flex; } @stillwind-slot utilities;");
        let source = Arc::new(
            SourceFile::new(
                FileId(1),
                "entry.tsx",
                "import { style } from 'stillwind/react'; export const x = style('flex');",
            )
            .unwrap(),
        );
        let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
        let options = CompilerOptions::default();
        let compiled = ModuleCompiler::new(design.clone(), options.clone()).compile(&analysis);
        assert!(compiled.is_success());
        let output = StylesheetRenderer::new(&design, &options, &Identifiers::builtin())
            .render_compilation(&compiled, source.full_span());
        assert!(output.diagnostics.is_empty());
        assert!(!output.module_css.is_empty());
        assert_eq!(output.module_css, compiled.css.to_css());
    }
    /// Authored CSS reached through an import keeps the physical file and byte range it
    /// came from, and the same stylesheet renders identically with and without mappings.
    #[test]
    fn imported_authored_css_and_theme_tokens_keep_their_physical_sources() {
        use stillwind_design_system::{
            CssSourceName, DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions,
        };

        use crate::{CompilerOptions, Identifiers, StylesheetRenderer};

        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        let entry = "@import 'child.css' layer(parts) supports(display:grid) screen;\n\
                     @theme { --color-brand: red; } :root,:host { @stillwind-slot theme; }";
        let DesignSystemLoadStep::NeedFile(requests) =
            loader.provide(0, CssSourceName::from("entry.css"), entry.into())
        else {
            panic!("import")
        };
        let child = "/* 日本語 🧭 */\n.imported { color:var(--color-brand); }";
        let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
            requests[0].id,
            CssSourceName::from("child.css"),
            child.into(),
        ) else {
            panic!("complete")
        };
        let output = StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::builtin(),
        )
        .render_base_stylesheet_with_map()
        .unwrap();
        assert_eq!(
            output.css,
            StylesheetRenderer::new(
                &snapshot,
                &CompilerOptions::default(),
                &Identifiers::builtin()
            )
            .render_base_stylesheet()
            .unwrap()
        );
        let authored = output
            .mappings
            .iter()
            .find(|mapping| mapping.source.path() == "child.css")
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
                .any(|mapping| mapping.source.path() == "entry.css"
                    && output.css[mapping.generated.clone()].contains("--color-brand"))
        );
        assert!(output.css.contains("@supports (display:grid)"));
    }
}
