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
    use stillwind_source::FileId;

    use super::*;

    fn source() -> Arc<SourceFile> {
        Arc::new(
            SourceFile::new(FileId(7), "日本語.css", "/* 🧭 */  .日本 { color: red; }").unwrap(),
        )
    }

    #[test]
    fn text_preserves_exact_authored_bytes_and_source_identity() {
        let source = source();
        let start = source.text().find("  .").unwrap();
        let span = Span::new(
            source.id(),
            u32::try_from(start).unwrap(),
            source.full_span().end.0,
        );
        let mut output = MappedStylesheet::default();
        output.text("/* 前置 */", None);
        let offset = output.css.len();
        output.text(source.slice(span).unwrap(), Some((source.clone(), span)));
        assert_eq!(output.mappings.len(), 1);
        let mapping = &output.mappings[0];
        assert!(mapping.exact);
        assert!(Arc::ptr_eq(&mapping.source, &source));
        assert_eq!(mapping.generated, offset..output.css.len());
        assert_eq!(mapping.original, start..source.text().len());
        assert_eq!(
            &output.css[mapping.generated.clone()],
            source.slice(span).unwrap()
        );
    }

    #[test]
    fn transformed_text_points_past_original_leading_whitespace() {
        let source = Arc::new(
            SourceFile::new(
                FileId(7),
                "theme.css",
                "/* 🧭 */\u{3000}\n  --color-brand: red;",
            )
            .unwrap(),
        );
        let start = "/* 🧭 */".len();
        let span = Span::new(
            source.id(),
            u32::try_from(start).unwrap(),
            source.full_span().end.0,
        );
        let mut output = MappedStylesheet::default();
        output.text("--color-brand:red;", Some((source.clone(), span)));
        let mapping = &output.mappings[0];
        assert!(!mapping.exact);
        assert_eq!(mapping.generated, 0.."--color-brand:red;".len());
        assert_eq!(
            mapping.original,
            source.text().find("--color-brand").unwrap()..source.text().len()
        );
        assert!(Arc::ptr_eq(&mapping.source, &source));
    }

    #[test]
    fn empty_and_invalid_origins_do_not_create_mappings_or_discard_text() {
        let source = Arc::new(SourceFile::new(FileId(7), "input.css", "日本").unwrap());
        let mut output = MappedStylesheet::default();
        output.text("", Some((source.clone(), Span::new(source.id(), 0, 6))));
        output.text("", Some((source.clone(), Span::new(source.id(), 0, 0))));
        for span in [
            Span::new(FileId(8), 0, 6),
            Span::new(source.id(), 0, 9),
            Span::new(source.id(), 1, 3),
            Span::new(source.id(), 3, 0),
        ] {
            output.text(".x{}", Some((source.clone(), span)));
        }
        output.text("/* unmapped */", None);
        assert_eq!(output.css, ".x{}.x{}.x{}.x{}/* unmapped */");
        assert!(output.mappings.is_empty());
    }

    #[test]
    fn append_offsets_all_generated_ranges_without_mutating_originals() {
        let source = source();
        let span = source.full_span();
        let mut first = MappedStylesheet::default();
        first.text(source.text(), Some((source.clone(), span)));
        let offset = first.css.len();
        let mut second = MappedStylesheet::default();
        second.text("/* 間 */", None);
        second.text(source.text(), Some((source.clone(), span)));
        second.text(".日本{color:red}", Some((source.clone(), span)));
        first.append(&second);
        assert_eq!(first.css, format!("{}{}", source.text(), second.css));
        assert_eq!(first.mappings.len(), 3);
        assert_eq!(first.mappings[0].generated, 0..offset);
        for (mapping, original) in first.mappings[1..].iter().zip(&second.mappings) {
            assert_eq!(
                mapping.generated,
                original.generated.start + offset..original.generated.end + offset
            );
            assert_eq!(mapping.original, original.original);
            assert_eq!(mapping.exact, original.exact);
            assert!(Arc::ptr_eq(&mapping.source, &original.source));
            assert_eq!(
                &first.css[mapping.generated.clone()],
                &second.css[original.generated.clone()]
            );
        }
        assert_eq!(second.mappings[0].generated.start, "/* 間 */".len());
    }

    #[test]
    fn prepend_unmapped_shifts_only_generated_byte_ranges() {
        let source = source();
        let span = source.full_span();
        let mut output = MappedStylesheet::default();
        output.text(source.text(), Some((source.clone(), span)));
        output.text(".日本{color:red}", Some((source.clone(), span)));
        let before = output.clone();
        let prefix = "/* 日本語 🧭 */";
        output.prepend_unmapped(prefix);
        assert_eq!(output.css, format!("{prefix}{}", before.css));
        assert_eq!(output.mappings.len(), before.mappings.len());
        for (mapping, original) in output.mappings.iter().zip(&before.mappings) {
            assert_eq!(
                mapping.generated,
                original.generated.start + prefix.len()..original.generated.end + prefix.len()
            );
            assert_eq!(mapping.original, original.original);
            assert_eq!(mapping.exact, original.exact);
            assert!(Arc::ptr_eq(&mapping.source, &original.source));
        }
        let mut empty = MappedStylesheet::default();
        empty.prepend_unmapped(prefix);
        assert_eq!(empty.css, prefix);
        assert!(empty.mappings.is_empty());
    }
}
