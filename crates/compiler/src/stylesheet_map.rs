//! Source locations retained while assembling CSS, before a host rewrites URLs.
//!
//! A [`MappedStylesheet`] pairs generated CSS with mappings back into the immutable source
//! files it came from. Generated ranges are byte offsets into the assembled CSS, original
//! ranges are byte offsets into one specific [`SourceFile`], and `exact` records whether the
//! generated text is byte-for-byte the authored slice or a rendering of it. Mappings are
//! appended in output order and survive [`MappedStylesheet::append`] and
//! semantic directive lowering, which adjusts only the generated side.

use std::ops::Range;
use std::sync::Arc;

use stillwind_source::{SourceFile, Span};

/// One generated range and the authored range it came from.
#[derive(Clone, Debug)]
pub struct StylesheetMapping {
    /// Byte range in the assembled CSS.
    pub generated: Range<usize>,
    /// Immutable buffer the original range addresses.
    pub source: Arc<SourceFile>,
    /// Byte range in `source`.
    pub original: Range<usize>,
    /// Byte-for-byte authored text; other mappings identify a generated rule's origin.
    pub exact: bool,
}

/// Generated CSS plus the mappings that explain where each part came from.
#[derive(Clone, Debug, Default)]
pub struct MappedStylesheet {
    /// The assembled CSS.
    pub css: String,
    /// Mappings in output order; ranges never overlap.
    pub mappings: Vec<StylesheetMapping>,
}

impl MappedStylesheet {
    /// Omits semantic fragment directives while retaining source coordinates.
    /// A rendered mapping split by an omission retains its full original origin;
    /// exact mappings instead retain the corresponding original byte slice.
    pub(crate) fn omit_ranges(&self, ranges: &[Range<usize>]) -> Self {
        let mut output = Self::default();
        let mut cursor = 0;
        for range in ranges
            .iter()
            .cloned()
            .chain(std::iter::once(self.css.len()..self.css.len()))
        {
            let end = range.start;
            let offset = output.css.len();
            output.css.push_str(&self.css[cursor..end]);
            for mapping in &self.mappings {
                let start = mapping.generated.start.max(cursor);
                let stop = mapping.generated.end.min(end);
                if start >= stop {
                    continue;
                }
                let mut kept = mapping.clone();
                kept.generated = offset + start - cursor..offset + stop - cursor;
                if kept.exact {
                    kept.original = mapping.original.start + start - mapping.generated.start
                        ..mapping.original.start + stop - mapping.generated.start;
                }
                output.mappings.push(kept);
            }
            cursor = range.end;
        }
        output
    }
    /// Appends another stylesheet, shifting its generated ranges by the current length.
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

    /// Appends `text`, recording a mapping when `origin` names a sliceable source range.
    ///
    /// An empty text or an unusable span drops only the mapping; the text is always
    /// appended, so a caller that passes no origin still gets the CSS. A mapping that is not
    /// `exact` starts after the original's leading whitespace, because generated text has no
    /// leading indentation for those bytes to point at.
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
    fn directive_omission_preserves_utf8_and_exact_and_rendered_origins() {
        let source = Arc::new(
            SourceFile::new(
                FileId(7),
                "日本.css",
                "前@stillwind-require-properties --x;後",
            )
            .unwrap(),
        );
        let mut output = MappedStylesheet::default();
        output.text(source.text(), Some((source.clone(), source.full_span())));
        let start = "前".len();
        let end = source.text().find('後').unwrap();
        let lowered = output.omit_ranges(std::slice::from_ref(&(start..end)));
        assert_eq!(lowered.css, "前後");
        assert_eq!(lowered.mappings.len(), 2);
        for mapping in &lowered.mappings {
            assert!(mapping.exact);
            assert_eq!(
                &lowered.css[mapping.generated.clone()],
                &source.text()[mapping.original.clone()]
            );
        }
        output.mappings[0].exact = false;
        let lowered = output.omit_ranges(std::slice::from_ref(&(start..end)));
        assert!(
            lowered
                .mappings
                .iter()
                .all(|mapping| mapping.original == (0..source.text().len()) && !mapping.exact)
        );
    }
}
