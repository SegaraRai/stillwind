use stillwind_design_system::DesignSystemSnapshot;
use stillwind_source::{CompileError, Span};

use crate::compiler::{
    render_mapped, render_module_stylesheet, render_react_types, render_stylesheet,
};
use crate::{CompileResult, CompiledModule, CompilerOptions, MappedStylesheet, StylesheetError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StylesheetPlacement {
    Document,
    Carrier,
}

pub struct StylesheetRenderer<'a> {
    snapshot: &'a DesignSystemSnapshot,
    options: CompilerOptions,
}

impl<'a> StylesheetRenderer<'a> {
    pub fn new(snapshot: &'a DesignSystemSnapshot, options: &CompilerOptions) -> Self {
        Self {
            snapshot,
            options: options.clone().resolved(snapshot),
        }
    }

    pub fn generate_react_types(&self) -> String {
        render_react_types(self.snapshot.theme())
    }

    /// Produces standalone compilation output, including document CSS.
    /// Assembly failures become diagnostics and invalidate the generated output.
    pub fn render_compilation(
        &self,
        module: &CompiledModule,
        diagnostic_span: Span,
    ) -> CompileResult {
        let mut result = module.to_module_output(self.snapshot);
        result.css.clear();
        if module.is_success() {
            match self.render_stylesheet(&result.module_css) {
                Ok(css) => result.css = css,
                Err(error) => {
                    result.diagnostics.push(CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        diagnostic_span,
                    ));
                    result.module_css.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
        }
        result
    }

    /// # Errors
    /// Returns an error when the assembled CSS cannot be analyzed.
    pub fn render_stylesheet(&self, module_css: &str) -> Result<String, StylesheetError> {
        render_stylesheet(self.snapshot.theme(), module_css, &self.options)
    }

    /// Document CSS with the support layer reserved before any late producer.
    ///
    /// # Errors
    /// Returns an error when the authored CSS cannot be analyzed.
    pub fn render_base_stylesheet(&self) -> Result<String, StylesheetError> {
        let mut css = self.render_stylesheet("")?;
        if !css.starts_with("@layer properties;") {
            css.insert_str(0, "@layer properties;\n");
        }
        Ok(css)
    }

    /// Renders one producer's CSS and required tokens, keyframes and properties.
    /// Authored global rules and Preflight belong to `render_base_stylesheet()`.
    ///
    /// # Errors
    /// Returns an error when the support closure cannot be analyzed.
    pub fn render_module_stylesheet(&self, module_css: &str) -> Result<String, StylesheetError> {
        render_module_stylesheet(self.snapshot.theme(), module_css, &self.options)
    }

    /// # Errors
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

    /// Document CSS with source provenance and a stable support-layer reservation.
    ///
    /// # Errors
    /// Returns invalid authored CSS.
    pub fn render_base_stylesheet_with_map(&self) -> Result<MappedStylesheet, StylesheetError> {
        let mut output = self.render_mapped_stylesheet(
            &MappedStylesheet::default(),
            StylesheetPlacement::Document,
        )?;
        if !output.css.starts_with("@layer properties;") {
            output.prepend_unmapped("@layer properties;\n");
        }
        Ok(output)
    }
}
