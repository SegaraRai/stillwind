//! Source highlights shared by errors and lint findings.

use serde::{Deserialize, Serialize};

use stillwind_source::Span;

/// One highlighted source range with an optional local explanation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    /// Half-open UTF-8 byte range in the identified reporting source. Producers must
    /// supply valid bounds; constructing a label does not validate them.
    pub span: Span,
    /// Text shown next to the range; `None` lets the diagnostic message stand for the
    /// label itself.
    pub message: Option<String>,
}
