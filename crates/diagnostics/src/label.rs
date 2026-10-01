//! Source highlights shared by errors and lint findings.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use stillwind_source::{Fingerprint, SourceFile, SourceRegistry, SourceRegistryError, Span};

/// One highlighted source range with an optional local explanation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    /// Half-open UTF-8 byte range in the identified reporting source. Producers must
    /// supply valid bounds; constructing a label does not validate them.
    pub span: Span,
    /// Text shown next to the range; `None` lets the diagnostic message stand for the
    /// label itself.
    pub message: Option<String>,
}

/// A diagnostic's primary highlight followed by optional related highlights.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Label>", into = "Vec<Label>")]
pub struct Labels(Vec<Label>);

impl Labels {
    /// Starts a highlight set with its mandatory primary location.
    pub fn new(primary: Label) -> Self {
        Self(vec![primary])
    }

    /// Adds a related location after the primary location.
    pub fn push(&mut self, label: Label) {
        self.0.push(label);
    }
}

impl std::ops::Deref for Labels {
    type Target = [Label];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Labels {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Labels {
    type Item = &'a Label;
    type IntoIter = std::slice::Iter<'a, Label>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Labels {
    type Item = &'a mut Label;
    type IntoIter = std::slice::IterMut<'a, Label>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl TryFrom<Vec<Label>> for Labels {
    type Error = &'static str;

    fn try_from(value: Vec<Label>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("a diagnostic needs a primary location")
        } else {
            Ok(Self(value))
        }
    }
}

impl From<Labels> for Vec<Label> {
    fn from(value: Labels) -> Self {
        value.0
    }
}

/// An edit of one original document, bound to its exact content revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEdit {
    /// Source snapshot and validated byte range to replace.
    pub range: SourceRange,
    /// Text to insert in place of the range.
    pub replacement: String,
}

/// One user-invoked fix consisting of one or more edits applied together.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixAction {
    /// Title shown to the user.
    pub title: String,
    /// Edits belonging to this action; validation rejects an empty list.
    pub edits: Vec<SourceEdit>,
}

/// Shared source context and possible fixes attached to one typed diagnostic cause.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticInfo {
    /// The first label is the mandatory primary location.
    pub labels: Labels,
    /// Additional explanatory text.
    pub notes: Vec<String>,
    /// Alternative user-invoked fixes.
    pub actions: Vec<FixAction>,
}

impl DiagnosticInfo {
    /// Attaches a primary byte span to a newly constructed diagnostic.
    pub fn new(span: Span) -> Self {
        Self {
            labels: Labels::new(Label {
                span,
                message: None,
            }),
            notes: Vec::new(),
            actions: Vec::new(),
        }
    }
}

/// A byte range bound to the immutable original document it addresses.
#[derive(Clone, Debug)]
pub struct SourceRange {
    source: Arc<SourceFile>,
    start: u32,
    end: u32,
}

impl SourceRange {
    /// Resolves a compilation span through the registry that issued its file ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown file ID or a range outside that source.
    pub fn from_span(registry: &SourceRegistry, span: Span) -> Result<Self, SourceRegistryError> {
        let source = registry.source(span.file)?.clone();
        source.slice(span)?;
        if source.uri().is_none() {
            return Err(SourceRegistryError::UnreportableSource(span.file));
        }
        Ok(Self {
            source,
            start: span.start.0,
            end: span.end.0,
        })
    }

    /// Returns the exact source snapshot used by this range.
    pub fn source(&self) -> &SourceFile {
        &self.source
    }

    /// Inclusive byte start.
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Exclusive byte end.
    pub const fn end(&self) -> u32 {
        self.end
    }
}

impl PartialEq for SourceRange {
    fn eq(&self, other: &Self) -> bool {
        self.source.uri() == other.source.uri()
            && self.source.text() == other.source.text()
            && self.start == other.start
            && self.end == other.end
    }
}

impl Eq for SourceRange {}

impl Serialize for SourceRange {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut value = serializer.serialize_struct("SourceRange", 4)?;
        value.serialize_field("uri", &self.source.uri())?;
        value.serialize_field("revision", &Fingerprint::of(self.source.text()).0)?;
        value.serialize_field("start", &self.start)?;
        value.serialize_field("end", &self.end)?;
        value.end()
    }
}
