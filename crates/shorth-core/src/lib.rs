//! Public orchestration API for the first compiler slice.

use shorth_source::{Diagnostic, SourceFile};

pub struct Compiler;

impl Compiler {
    pub fn check_javascript(source: &SourceFile) -> Vec<Diagnostic> {
        shorth_js_oxc::parse(source).diagnostics
    }

    pub fn analyze_javascript(source: &SourceFile) -> shorth_js_oxc::ParsedJavaScript {
        shorth_js_oxc::parse(source)
    }

    /// Reads the project's CSS-first theme surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the stylesheet is invalid or contains an
    /// unterminated `@theme` block.
    pub fn read_theme(
        source: &SourceFile,
    ) -> Result<shorth_css::ThemeIndex, shorth_css::ThemeError> {
        shorth_css::parse_theme(source)
    }
}

#[cfg(test)]
mod tests {
    use shorth_source::FileId;

    use super::*;

    #[test]
    fn exposes_the_first_frontends_through_one_compiler_api() {
        let js = SourceFile::new(FileId(1), "App.tsx", "export const App = () => <main />;");
        let css = SourceFile::new(FileId(2), "shorth.css", "@theme { --color-accent: red; }");

        assert!(Compiler::check_javascript(&js).is_empty());
        assert_eq!(Compiler::read_theme(&css).unwrap().tokens.len(), 1);
    }
}
