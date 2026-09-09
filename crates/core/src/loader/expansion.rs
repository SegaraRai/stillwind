use super::{CssSourceMapping, SourceFile, Span};

#[derive(Clone, Debug, Default)]
pub(super) struct ExpandedSource {
    pub(super) text: String,
    pub(super) mappings: Vec<CssSourceMapping>,
}

impl ExpandedSource {
    pub(super) fn append_import(&mut self, other: &Self, wrappers: &[String]) {
        for wrapper in wrappers {
            self.text.push_str(wrapper);
            self.text.push('{');
        }
        self.append(other);
        for _ in wrappers {
            self.text.push('}');
        }
    }

    pub(super) fn append_source(&mut self, source: &SourceFile, range: std::ops::Range<usize>) {
        let start = self.text.len();
        self.text.push_str(&source.text()[range.clone()]);
        self.mappings.push(CssSourceMapping {
            generated: start..self.text.len(),
            file: source.id(),
            source_start: range.start,
        });
    }

    pub(super) fn append(&mut self, other: &Self) {
        let offset = self.text.len();
        self.text.push_str(&other.text);
        self.mappings
            .extend(other.mappings.iter().map(|mapping| CssSourceMapping {
                generated: mapping.generated.start + offset..mapping.generated.end + offset,
                file: mapping.file,
                source_start: mapping.source_start,
            }));
    }

    pub(super) fn original_span(&self, span: Span) -> Option<Span> {
        let start = span.start.0 as usize;
        let end = span.end.0 as usize;
        self.mappings
            .iter()
            .find(|mapping| mapping.generated.start <= start && end <= mapping.generated.end)
            .map(|mapping| {
                let start = mapping.source_start + start - mapping.generated.start;
                Span::new(
                    mapping.file,
                    u32::try_from(start).expect("source offset"),
                    u32::try_from(start + span.len() as usize).expect("source offset"),
                )
            })
    }
}
