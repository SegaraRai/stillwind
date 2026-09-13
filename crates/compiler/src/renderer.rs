//! Stylesheet rendering entry points over one immutable snapshot.
//!
//! [`StylesheetRenderer`] assembles CSS from the design system's output template: it
//! expands authored directives, selects the theme tokens, keyframes, and `@property`
//! registrations the CSS actually needs, and walks the template's layers and slots.
//! [`StylesheetPlacement`] decides whether authored document rules and Preflight
//! participate, so a per-module carrier does not duplicate them. Assembly failures come
//! back as [`StylesheetError`], or, when rendering a whole compilation, as a `SHCSS001`
//! diagnostic that invalidates the generated output instead of shipping CSS known to be
//! incomplete.

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_source::{CompileError, Span};

use crate::compiler::{
    render_mapped, render_module_stylesheet, render_react_types, render_stylesheet,
};
use crate::{CompileResult, CompiledModule, CompilerOptions, MappedStylesheet, StylesheetError};

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
}

impl<'a> StylesheetRenderer<'a> {
    /// Creates a renderer for `snapshot`.
    ///
    /// The options' prefixes are resolved against the snapshot, so rendered text always
    /// agrees with the theme being rendered.
    pub fn new(snapshot: &'a DesignSystemSnapshot, options: &CompilerOptions) -> Self {
        Self {
            snapshot,
            options: options.clone().resolved(snapshot),
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
    /// and source edits and records a diagnostic.
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
                    result.diagnostics.push(CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        diagnostic_span,
                    ));
                    result.module_css.clear();
                    result.contributions.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
        }
        result
    }

    /// Assembles document CSS for raw utility rules.
    /// Generated artifacts must use [`Self::render_mapped_stylesheet`] to retain
    /// keyframe placement and explicit registration dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when the assembled CSS cannot be analyzed.
    pub fn render_stylesheet(&self, module_css: &str) -> Result<String, StylesheetError> {
        render_stylesheet(self.snapshot.theme(), module_css, &self.options)
    }

    /// Document CSS with the layer reservations authored by the entry stylesheet.
    ///
    /// The default package reserves support order before any late producer; a custom
    /// entry owns its own layer order. Rendering never adds an implicit reservation.
    ///
    /// # Errors
    ///
    /// Returns an error when the authored CSS cannot be analyzed.
    pub fn render_base_stylesheet(&self) -> Result<String, StylesheetError> {
        self.render_stylesheet("")
    }

    /// Renders one producer's CSS and required tokens, keyframes and properties.
    /// The output is a carrier fragment: authored document rules and Preflight are
    /// included only when the placement is [`StylesheetPlacement::Document`], so this
    /// form is for a host that transports one module's CSS on its own.
    ///
    /// # Errors
    ///
    /// Returns an error when the support closure cannot be analyzed.
    pub fn render_module_stylesheet(&self, module_css: &str) -> Result<String, StylesheetError> {
        render_module_stylesheet(self.snapshot.theme(), module_css, &self.options)
    }

    /// Renders a producer's CSS with source provenance attached.
    ///
    /// # Errors
    ///
    /// Returns invalid CSS support closure or missing utility slots.
    pub fn render_mapped_stylesheet(
        &self,
        module_css: &MappedStylesheet,
        placement: StylesheetPlacement,
    ) -> Result<MappedStylesheet, StylesheetError> {
        render_mapped(
            &self.snapshot.theme().output_template,
            self.snapshot.theme(),
            module_css,
            &self.options,
            placement == StylesheetPlacement::Document,
            Some(self.snapshot.sources()),
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
    pub fn render_base_stylesheet_with_map(&self) -> Result<MappedStylesheet, StylesheetError> {
        self.render_mapped_stylesheet(&MappedStylesheet::default(), StylesheetPlacement::Document)
    }
}
