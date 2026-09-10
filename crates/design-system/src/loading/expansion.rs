use stillwind_css::relative_css_urls;
use stillwind_source::{Fingerprint, SourceError, SourceFile, Span};

use super::protocol::{CssResourceReference, CssSourceMapping};

#[derive(Clone, Debug, Default)]
pub(super) struct ExpandedSource {
    pub(super) text: String,
    pub(super) mappings: Vec<CssSourceMapping>,
    pub(super) resources: Vec<CssResourceReference>,
}

impl ExpandedSource {
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

    pub(super) fn append_source(
        &mut self,
        source: &SourceFile,
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
                "{}:{}{}",
                source.path().len(),
                source.path(),
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
                owner: source.path().into(),
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
        let source = SourceFile::new(FileId(9), "nested/design.css", text).unwrap();
        let mut output = ExpandedSource::default();
        output.append_source(&source, 0..text.len()).unwrap();
        assert_eq!(output.resources.len(), 1);
        assert_eq!(output.resources[0].owner, "nested/design.css");
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
        let source = SourceFile::new(FileId(1), "x", "é").unwrap();
        let mut output = ExpandedSource::default();
        assert!(output.append_source(&source, 0..1).is_err());
        assert!(output.text.is_empty());
        output.append_source(&source, 0..2).unwrap();
        assert!(output.original_span(Span::new(FileId(1), 2, 1)).is_none());
        output.mappings[0].source_start = usize::MAX;
        assert!(output.original_span(Span::new(FileId(1), 1, 2)).is_none());
    }

    #[test]
    fn imported_wrappers_shift_mappings_without_mapping_synthetic_text() {
        let source = SourceFile::new(FileId(7), "a.css", "color:red;").unwrap();
        let mut fragment = ExpandedSource::default();
        fragment.append_source(&source, 0..10).unwrap();
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
