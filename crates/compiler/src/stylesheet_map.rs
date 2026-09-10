//! Source locations retained while assembling CSS, before a host rewrites URLs.

use std::ops::Range;
use std::sync::Arc;

use stillwind_source::{SourceFile, Span};

#[derive(Clone, Debug)]
pub struct StylesheetMapping {
    pub generated: Range<usize>,
    pub source: Arc<SourceFile>,
    pub original: Range<usize>,
    /// Byte-for-byte authored text; other mappings identify a generated rule's origin.
    pub exact: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MappedStylesheet {
    pub css: String,
    pub mappings: Vec<StylesheetMapping>,
}

impl MappedStylesheet {
    pub(crate) fn append(&mut self, other: &Self) {
        let offset = self.css.len();
        self.css.push_str(&other.css);
        self.mappings
            .extend(other.mappings.iter().cloned().map(|mut mapping| {
                mapping.generated.start += offset;
                mapping.generated.end += offset;
                mapping
            }));
    }

    pub fn text(&mut self, text: &str, origin: Option<(Arc<SourceFile>, Span)>) {
        let start = self.css.len();
        self.css.push_str(text);
        if let Some((source, span)) = origin {
            let Ok(original) = source.slice(span) else {
                return;
            };
            let exact = text == original;
            let leading = if exact {
                0
            } else {
                original.len() - original.trim_start().len()
            };
            if !text.is_empty() {
                self.mappings.push(StylesheetMapping {
                    generated: start..self.css.len(),
                    original: span.start.0 as usize + leading..span.end.0 as usize,
                    source,
                    exact,
                });
            }
        }
    }

    pub(crate) fn prepend_unmapped(&mut self, text: &str) {
        self.css.insert_str(0, text);
        for mapping in &mut self.mappings {
            mapping.generated.start += text.len();
            mapping.generated.end += text.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{
        CssSourceName, DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions,
    };

    use crate::{CompilerOptions, StylesheetRenderer};

    #[test]
    fn imported_authored_css_and_theme_tokens_keep_their_physical_sources() {
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        let DesignSystemLoadStep::NeedFile(requests) = loader.provide(0, CssSourceName::from("entry.css"), "@import 'child.css' layer(parts) supports(display:grid) screen;\n@theme { --color-brand: red; }".into()) else { panic!("import") };
        let child = "/* 日本語 🧭 */\n.imported { color:var(--color-brand); }";
        let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
            requests[0].id,
            CssSourceName::from("child.css"),
            child.into(),
        ) else {
            panic!("complete")
        };
        let output = StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
            .render_base_stylesheet_with_map()
            .unwrap();
        assert_eq!(
            output.css,
            StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
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
