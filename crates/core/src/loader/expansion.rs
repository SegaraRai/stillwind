use stillwind_source::{SourceError, SourceFile, Span};

use super::protocol::CssSourceMapping;

#[derive(Clone, Debug, Default)]
pub(super) struct ExpandedSource {
    pub(super) text: String,
    pub(super) mappings: Vec<CssSourceMapping>,
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
        let start = self.text.len();
        let text = source
            .text()
            .get(range.clone())
            .ok_or(SourceError::InvalidRange {
                start: range.start,
                end: range.end,
            })?;
        self.append_text(text)?;
        self.mappings.push(CssSourceMapping {
            generated: start..self.text.len(),
            file: source.id(),
            source_start: range.start,
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
            }));
        Ok(())
    }

    pub(super) fn original_span(&self, span: Span) -> Option<Span> {
        let start = span.start.0 as usize;
        let end = span.end.0 as usize;
        self.mappings
            .iter()
            .find(|mapping| mapping.generated.start <= start && end <= mapping.generated.end)
            .and_then(|mapping| {
                let start = mapping
                    .source_start
                    .checked_add(start.checked_sub(mapping.generated.start)?)?;
                Some(Span::new(
                    mapping.file,
                    u32::try_from(start).ok()?,
                    u32::try_from(start.checked_add(span.len()? as usize)?).ok()?,
                ))
            })
    }
}

#[cfg(test)]
mod tests {
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

    use super::*;
    use stillwind_source::FileId;

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
