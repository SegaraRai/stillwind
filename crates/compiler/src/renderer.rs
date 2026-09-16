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
    detached_symbol, render_mapped, render_module_stylesheet, render_react_types, render_stylesheet,
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
    /// agrees with the theme being rendered. `identifiers` names every generated helper and
    /// anonymous layer this renderer writes.
    pub fn new(
        snapshot: &'a DesignSystemSnapshot,
        options: &CompilerOptions,
        identifiers: &Identifiers,
    ) -> Self {
        Self {
            snapshot,
            options: options.clone().resolved(snapshot),
            identifiers: identifiers.context(),
        }
    }

    /// Ambient TypeScript declarations for the framework vocabulary of this theme.
    pub fn generate_react_types(&self) -> String {
        render_react_types(self.snapshot.theme())
    }

    /// Produces standalone compilation output, including document CSS.
    /// Assembly failures become diagnostics and invalidate the generated output.
    ///
    /// On success `css` holds the assembled document stylesheet and `module_css`
    /// retains the unwrapped module rules. Assembly failure clears both output rules
    /// and source edits and records a diagnostic at `diagnostic_span`. That span must be
    /// valid in the caller's reporting source; it is used without coordinate validation.
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
        // TODO: this is the only entry point that drains the rejections. A host that renders a
        // carrier or a project stylesheet with its own generator never learns which of its names
        // were rejected; return them from those entry points too.
        result.rejected_names = self.identifiers.drain_rejected().into_entries();
        // Helpers written for authored CSS and anonymous layers belong to no declaration, so they
        // are reported under the stylesheet that named them.
        result.symbols.extend(self.take_detached_symbols());
        result
    }

    /// Takes the names this renderer produced that belong to no declaration.
    ///
    /// A renderer holds one compilation's name allocation, so this drains what its renders since
    /// the last call produced: conditional helpers written for authored CSS or theme fragments, and
    /// anonymous layers. They are reported under the stylesheet that named them and carry no
    /// declaration position, so a document-wide pool check can compare them against every other
    /// name the document uses.
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
