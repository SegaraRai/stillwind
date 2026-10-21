//! Source locations retained while assembling CSS, before a host rewrites URLs.
//!
//! A [`MappedStylesheet`] pairs generated CSS with mappings back into the immutable
//! source files it came from. Generated ranges are byte offsets into the assembled CSS,
//! original ranges are byte offsets into one specific [`SourceFile`], and `exact`
//! records whether the generated text is byte-for-byte the authored slice or a
//! rendering of it. Mappings are appended in output order and survive
//! [`MappedStylesheet::append`] and slot placement. Keyframes retain their own
//! coordinates until appended at their slot.

use std::ops::Range;
use std::sync::Arc;

use lean_string::{LeanStr, LeanString};

use stillwind_source::{SourceFile, Span};

/// One generated range and the authored range it came from.
#[derive(Clone, Debug)]
pub struct StylesheetMapping {
    /// Half-open UTF-8 byte range in the containing stylesheet's `css`.
    pub generated: Range<usize>,
    /// Immutable buffer the original range addresses.
    pub source: Arc<SourceFile>,
    /// Half-open UTF-8 byte range in `source`.
    pub original: Range<usize>,
    /// Byte-for-byte authored text; other mappings identify a generated rule's origin.
    pub exact: bool,
}

/// Generated CSS plus the mappings that explain where each part came from.
#[derive(Clone, Debug, Default)]
pub struct MappedStylesheet {
    /// Template node identity for each emitted range, stable across producer
    /// selections.
    pub segments: Vec<(Range<usize>, usize)>,
    /// Generated `@keyframes` contributions in emission order. Each entry's mappings
    /// address that entry's own `css`, not the parent's rule text. Producers use leaf
    /// entries; the template assembler consumes this level of contributions.
    pub keyframes: Vec<Self>,
    /// Explicit custom-property registration names. Accumulation preserves encounter
    /// order and permits duplicates; registration selection treats them as a set.
    pub required_properties: Vec<LeanStr>,
    /// Rule text before slot assembly, or complete CSS after rendering. Pending
    /// keyframes in `keyframes` are not part of this string.
    pub css: String,
    /// Mappings in output order; ranges never overlap.
    pub mappings: Vec<StylesheetMapping>,
}

impl MappedStylesheet {
    /// Applies edits to the current CSS and rebases its source mappings and template
    /// segments.
    ///
    /// Each `(range, replacement)` addresses the original `self.css` in UTF-8 bytes.
    /// Ranges must be valid, ordered, and nonoverlapping. Empty ranges insert text, and
    /// multiple insertions at one offset retain their supplied order. An insertion at
    /// the start of a replacement must precede that replacement in `edits`.
    ///
    /// Retained text keeps clipped mappings. Replacement text receives an inexact
    /// mapping and segment identity only where an original range contains the edit's
    /// start. An exact source mapping anchors replacement text at the corresponding
    /// source byte. Insertion at EOF inherits no origin. Keyframes and requirements are
    /// unchanged. Input mappings must satisfy their range and exactness invariants.
    ///
    /// # Panics
    ///
    /// Invalid edit ordering or UTF-8 bounds can panic during slicing. Invalid mapping
    /// coordinates can panic during offset arithmetic. `self` is replaced only after
    /// the complete edited result has been assembled.
    pub(crate) fn replace_ranges(&mut self, edits: &[(Range<usize>, LeanString)]) {
        if edits.is_empty() {
            return;
        }
        let mut result = Self::default();
        let mut cursor = 0;
        for (range, replacement) in edits {
            self.copy_range(&mut result, cursor..range.start);
            let start = result.css.len();
            result.css.push_str(replacement);
            if !replacement.is_empty() {
                if let Some(mapping) = self
                    .mappings
                    .iter()
                    .find(|mapping| mapping.generated.contains(&range.start))
                {
                    let mut mapping = mapping.clone();
                    if mapping.exact {
                        mapping.original.start += range.start - mapping.generated.start;
                    }
                    mapping.generated = start..result.css.len();
                    mapping.exact = false;
                    result.mappings.push(mapping);
                }
                if let Some((_, position)) = self
                    .segments
                    .iter()
                    .find(|(span, _)| span.contains(&range.start))
                {
                    result.segments.push((start..result.css.len(), *position));
                }
            }
            cursor = range.end;
        }
        self.copy_range(&mut result, cursor..self.css.len());
        self.css = result.css;
        self.mappings = result.mappings;
        self.segments = result.segments;
    }

    /// Appends a retained slice of `self.css` and its intersecting mappings to
    /// `output`.
    ///
    /// `range` must be a valid half-open UTF-8 byte range in `self.css`. Both
    /// stylesheets must have valid generated ranges, and exact mappings must cover
    /// equal-length source and generated slices. Existing output mappings must end
    /// within its CSS. Generated ranges are clipped to the slice and shifted by the
    /// previous output length. Exact mappings also clip their source ranges, while
    /// inexact mappings retain their original source attribution. Segment identities
    /// follow retained text. Keyframes and requirements are not copied.
    ///
    /// # Panics
    ///
    /// Panics if `range` is invalid for slicing. Invalid mapping coordinates can panic
    /// during offset arithmetic after text has already been appended to `output`.
    fn copy_range(&self, output: &mut Self, range: Range<usize>) {
        let offset = output.css.len();
        output.css.push_str(&self.css[range.clone()]);
        for mapping in &self.mappings {
            let start = mapping.generated.start.max(range.start);
            let end = mapping.generated.end.min(range.end);
            if start >= end {
                continue;
            }
            let mut clipped = mapping.clone();
            if clipped.exact {
                clipped.original = mapping.original.start + start - mapping.generated.start
                    ..mapping.original.start + end - mapping.generated.start;
            }
            clipped.generated = offset + start - range.start..offset + end - range.start;
            output.mappings.push(clipped);
        }
        for (segment, position) in &self.segments {
            let start = segment.start.max(range.start);
            let end = segment.end.min(range.end);
            if start < end {
                output.segments.push((
                    offset + start - range.start..offset + end - range.start,
                    *position,
                ));
            }
        }
    }

    /// Appends another stylesheet's text, mappings, and pending contributions.
    ///
    /// Rule mappings shift by the previous UTF-8 byte length of `self.css`.
    /// `@keyframes` entries keep their own local coordinates. Dependency and
    /// `@keyframes` lists are extended in order without deduplication; `other` is not
    /// modified.
    pub(crate) fn append(&mut self, other: &Self) {
        self.keyframes.extend(other.keyframes.iter().cloned());
        self.required_properties
            .extend(other.required_properties.iter().cloned());
        let offset = self.css.len();
        self.segments.extend(
            other
                .segments
                .iter()
                .map(|(range, slot)| (range.start + offset..range.end + offset, *slot)),
        );
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
    /// Always extends `self.css`; empty text or an unusable origin adds no mapping.
    /// When `text` equals the source slice, the mapping is exact. Otherwise it
    /// attributes the generated text to the source span after its leading whitespace,
    /// preserving the declaration or rule's origin rather than claiming byte-for-byte
    /// alignment. Pending keyframes and dependency names are unaffected.
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
    use lean_string::ToLeanString;

    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn edits_preserve_utf8_origins_and_template_segments() {
        let source = Arc::new(SourceFile::new(FileId(7), None, None, ".日本{color:red}").unwrap());
        let mut css = MappedStylesheet::default();
        css.text(source.text(), Some((source.clone(), source.full_span())));
        css.segments.push((0..css.css.len(), 4));
        let color = css.css.find("red").unwrap();
        css.replace_ranges(&[
            (0..0, "@supports (--123:0){".into()),
            (color..color + 3, "blue".into()),
            (css.css.len()..css.css.len(), "}".into()),
        ]);
        assert_eq!(css.css, "@supports (--123:0){.日本{color:blue}}");
        assert!(css.segments.iter().all(|(_, position)| *position == 4));
        let mut end = 0;
        for mapping in &css.mappings {
            assert!(mapping.generated.start >= end);
            end = mapping.generated.end;
            assert!(end <= css.css.len());
            if mapping.exact {
                assert_eq!(
                    &css.css[mapping.generated.clone()],
                    &source.text()[mapping.original.clone()]
                );
            }
        }
        assert!(css.mappings.iter().any(|mapping| mapping.exact && css.css[mapping.generated.clone()].contains("日本")));
    }

    #[test]
    fn text_preserves_exact_authored_bytes_and_source_identity() {
        let source = Arc::new(
            SourceFile::new(FileId(7), None, None, "/* 🧭 */  .日本 { color: red; }").unwrap(),
        );
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
                None,
                None,
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
        let source = Arc::new(SourceFile::new(FileId(7), None, None, "日本").unwrap());
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
        let source = Arc::new(
            SourceFile::new(FileId(7), None, None, "/* 🧭 */  .日本 { color: red; }").unwrap(),
        );
        let span = source.full_span();
        let mut first = MappedStylesheet::default();
        first.text(source.text(), Some((source.clone(), span)));
        let offset = first.css.len();
        let mut second = MappedStylesheet::default();
        second.text("/* 間 */", None);
        second.text(source.text(), Some((source.clone(), span)));
        second.text(".日本{color:red}", Some((source.clone(), span)));
        first.append(&second);
        assert_eq!(
            first.css,
            format_args!("{}{}", source.text(), second.css).to_lean_string()
        );
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
}
