//! Import expansion and generated-to-original span mapping.
//!
//! Expansion inlines every supplied source into one buffer in authored import order and
//! wraps each imported fragment in the conditionals its `@import` carried. Substitution
//! and wrapping change text length, so the expansion records an explicit mapping run
//! per emitted region; [`ExpandedSource::original_span`] walks those runs to translate
//! a span in the expanded buffer back to the buffer the host supplied.
//!
//! Two invariants keep the mapping honest. Synthetic wrapper text produces no mapping,
//! so a span that lands inside a wrapper has no original location rather than a guessed
//! one, and a span may only cross mapping runs that stay in one file and remain
//! contiguous in the original. A resource reference is replaced by a placeholder that
//! the host later substitutes; that run records both original endpoints because its
//! length is not preserved.

use stillwind_css::relative_css_urls;
use stillwind_source::{CssSourceName, Fingerprint, SourceError, SourceFile, Span};

use super::protocol::{CssResourceReference, CssSourceMapping};

/// One source's inlined text plus the mapping and resources it produced.
#[derive(Clone, Debug, Default)]
pub(super) struct ExpandedSource {
    /// Inlined CSS including synthetic import wrappers and URL placeholders; limited to
    /// [`u32::MAX`] UTF-8 bytes.
    pub(super) text: String,
    /// Mapping runs in emitted byte order; synthetic wrappers have no run.
    pub(super) mappings: Vec<CssSourceMapping>,
    /// Resource references in traversal order, identifying placeholders in text.
    pub(super) resources: Vec<CssResourceReference>,
}

impl ExpandedSource {
    /// Appends text and fails if the result would exceed the 32-bit source limit,
    /// keeping the buffer usable for `SourceFile` construction later.
    fn append_text(&mut self, text: &str) -> Result<(), SourceError> {
        let end = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or(SourceError::TooLarge(usize::MAX))?;
        u32::try_from(end).map_err(|_| SourceError::TooLarge(end))?;
        self.text.push_str(text);
        Ok(())
    }

    /// Appends another expansion wrapped in the import's conditionals, in the authored
    /// modifier order, so `@layer` stays outside `@supports` and `@media`.
    ///
    /// The wrapper text is synthetic and intentionally unmapped; only the nested
    /// expansion keeps its mappings, shifted by the current offset.
    pub(super) fn append_import(
        &mut self,
        other: &Self,
        wrappers: &[String],
    ) -> Result<(), SourceError> {
        for wrapper in wrappers {
            self.append_text(wrapper)?;
            self.append_text("{")?;
        }
        self.append(other)?;
        for _ in wrappers {
            self.append_text("}")?;
        }
        Ok(())
    }

    /// Copies `range` of `source` verbatim, replacing every relative resource reference
    /// with a stable placeholder and recording it in `resources`.
    ///
    /// The placeholder identity mixes the owner name with the specifier so two sources
    /// that reference the same specifier stay distinguishable, and it is hash-based so
    /// repeated loads produce identical generated CSS.
    pub(super) fn append_source(
        &mut self,
        source: &SourceFile,
        name: &CssSourceName,
        range: std::ops::Range<usize>,
    ) -> Result<(), SourceError> {
        let text = source
            .text()
            .get(range.clone())
            .ok_or(SourceError::InvalidRange {
                start: range.start,
                end: range.end,
            })?;
        let mut cursor = range.start;
        for resource in relative_css_urls(text) {
            let start = range.start + resource.range.start;
            let end = range.start + resource.range.end;
            self.append_original(source, cursor..start)?;
            let identity = format!(
                "stillwind/resource/css/v1\0{}\0{}\0",
                name.as_str(),
                resource.specifier
            );
            let placeholder = format!("__stillwind_resource_{}__", Fingerprint::of(identity).0);
            let replacement = if resource.string {
                format!("\"{placeholder}\"")
            } else {
                format!("url(\"{placeholder}\")")
            };
            let generated_start = self.text.len();
            self.append_text(&replacement)?;
            self.mappings.push(CssSourceMapping {
                generated: generated_start..self.text.len(),
                file: source.id(),
                source_start: start,
                source_end: Some(end),
            });
            self.resources.push(CssResourceReference {
                placeholder,
                source_name: name.clone(),
                specifier: resource.specifier,
                span: Span::new(
                    source.id(),
                    u32::try_from(start).map_err(|_| SourceError::TooLarge(start))?,
                    u32::try_from(end).map_err(|_| SourceError::TooLarge(end))?,
                ),
            });
            cursor = end;
        }
        self.append_original(source, cursor..range.end)
    }

    fn append_original(
        &mut self,
        source: &SourceFile,
        range: std::ops::Range<usize>,
    ) -> Result<(), SourceError> {
        if range.is_empty() {
            return Ok(());
        }
        let start = self.text.len();
        self.append_text(&source.text()[range.clone()])?;
        self.mappings.push(CssSourceMapping {
            generated: start..self.text.len(),
            file: source.id(),
            source_start: range.start,
            source_end: None,
        });
        Ok(())
    }

    /// Appends another expansion and shifts its mappings by the current length.
    ///
    /// Mappings are validated against the appended text first, so a caller never has to
    /// reason about a half-registered mapping run.
    pub(super) fn append(&mut self, other: &Self) -> Result<(), SourceError> {
        let offset = self.text.len();
        for mapping in &other.mappings {
            if mapping.generated.start > mapping.generated.end
                || mapping.generated.end > other.text.len()
            {
                return Err(SourceError::InvalidRange {
                    start: mapping.generated.start,
                    end: mapping.generated.end,
                });
            }
        }
        self.append_text(&other.text)?;
        self.mappings
            .extend(other.mappings.iter().map(|mapping| CssSourceMapping {
                generated: mapping.generated.start + offset..mapping.generated.end + offset,
                file: mapping.file,
                source_start: mapping.source_start,
                source_end: mapping.source_end,
            }));
        self.resources.extend(other.resources.iter().cloned());
        Ok(())
    }

    /// Translates a span in this expanded text into the source buffer it came from, or
    /// `None` when the span is not attributable.
    ///
    /// A length-changing substitution maps to its full authored expression, including
    /// queries into its interior. `None` covers synthetic wrapper text and ranges that
    /// cross a file boundary or a gap in the original, reversed spans, and coordinates
    /// beyond 32 bits. Callers must treat a missing mapping as "no original location"
    /// rather than falling back to the generated offsets.
    pub(super) fn original_span(&self, span: Span) -> Option<Span> {
        let start = span.start.0 as usize;
        let end = span.end.0 as usize;
        if end < start {
            return None;
        }
        let first = self.mappings.iter().position(|mapping| {
            mapping.generated.start <= start && start < mapping.generated.end
        })?;
        let initial = &self.mappings[first];
        let original_start = if initial.source_end.is_some() {
            initial.source_start
        } else {
            initial
                .source_start
                .checked_add(start - initial.generated.start)?
        };
        let mut previous_end = None;
        let mut generated_end = initial.generated.start;
        for mapping in &self.mappings[first..] {
            if mapping.file != initial.file
                || mapping.generated.start != generated_end
                || previous_end.is_some_and(|previous| previous != mapping.source_start)
            {
                return None;
            }
            let original_end = mapping
                .source_end
                .or_else(|| mapping.source_start.checked_add(mapping.generated.len()))?;
            if end <= mapping.generated.end {
                let original_end = if mapping.source_end.is_some() {
                    original_end
                } else {
                    mapping
                        .source_start
                        .checked_add(end.checked_sub(mapping.generated.start)?)?
                };
                return Some(Span::new(
                    initial.file,
                    u32::try_from(original_start).ok()?,
                    u32::try_from(original_end).ok()?,
                ));
            }
            previous_end = Some(original_end);
            generated_end = mapping.generated.end;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn resource_substitution_preserves_spans_across_length_changes() {
        let text = "--image:linear-gradient(red,blue),url('é.png');--color:red;";
        let source = SourceFile::new(FileId(9), None, None, text).unwrap();
        let mut output = ExpandedSource::default();
        output
            .append_source(
                &source,
                &CssSourceName::from("nested/design.css"),
                0..text.len(),
            )
            .unwrap();
        assert_eq!(output.resources.len(), 1);
        assert_eq!(
            output.resources[0].source_name.as_str(),
            "nested/design.css"
        );
        assert_eq!(output.resources[0].specifier, "é.png");
        assert_eq!(
            source.slice(output.resources[0].span).unwrap(),
            "url('é.png')"
        );
        let value_end = output.text.find(';').unwrap();
        assert_eq!(
            output.original_span(Span::new(FileId(0), 8, u32::try_from(value_end).unwrap())),
            Some(Span::new(
                FileId(9),
                8,
                u32::try_from(text.find(';').unwrap()).unwrap()
            ))
        );
        let color = output.text.find("--color").unwrap();
        assert_eq!(
            output.original_span(Span::new(
                FileId(0),
                u32::try_from(color).unwrap(),
                u32::try_from(color + 7).unwrap()
            )),
            Some(Span::new(
                FileId(9),
                u32::try_from(text.find("--color").unwrap()).unwrap(),
                u32::try_from(text.find("--color").unwrap() + 7).unwrap()
            ))
        );
    }

    #[test]
    fn rejects_invalid_source_ranges_and_unrepresentable_mappings() {
        let source = SourceFile::new(FileId(1), None, None, "é").unwrap();
        let mut output = ExpandedSource::default();
        assert!(
            output
                .append_source(&source, &CssSourceName::from("test.css"), 0..1)
                .is_err()
        );
        assert!(output.text.is_empty());
        output
            .append_source(&source, &CssSourceName::from("test.css"), 0..2)
            .unwrap();
        assert!(output.original_span(Span::new(FileId(1), 2, 1)).is_none());
        output.mappings[0].source_start = usize::MAX;
        assert!(output.original_span(Span::new(FileId(1), 1, 2)).is_none());
    }

    #[test]
    fn imported_wrappers_shift_mappings_without_mapping_synthetic_text() {
        let source = SourceFile::new(FileId(7), None, None, "color:red;").unwrap();
        let mut fragment = ExpandedSource::default();
        fragment
            .append_source(&source, &CssSourceName::from("test.css"), 0..10)
            .unwrap();
        let mut expanded = ExpandedSource::default();
        expanded
            .append_import(&fragment, &["@layer a".into()])
            .unwrap();
        assert_eq!(expanded.text, "@layer a{color:red;}");
        assert_eq!(
            expanded.original_span(Span::new(FileId(0), 9, 14)),
            Some(Span::new(FileId(7), 0, 5))
        );
        assert_eq!(expanded.original_span(Span::new(FileId(0), 0, 2)), None);
        assert_eq!(expanded.original_span(Span::new(FileId(0), 9, 20)), None);
    }
}
