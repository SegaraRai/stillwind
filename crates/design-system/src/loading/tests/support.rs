//! Shared source builders and result helpers for these contracts.

use super::*;

/// Builds a response the way a host would after reading a file. The loader never
/// inspects the name, so a test may use any string it likes.
pub(super) fn input(id: u32, name: &str, content: &str) -> CssFileInput {
    CssFileInput {
        id,
        name: CssSourceName::from(name),
        uri: None,
        source_map_source: None,
        content: content.into(),
        lint: false,
    }
}

pub(super) fn requests(step: DesignSystemLoadStep) -> Vec<CssFileRequest> {
    match step {
        DesignSystemLoadStep::NeedFile(requests) => requests,
        other @ DesignSystemLoadStep::Complete(_) => {
            panic!("expected outstanding requests: {other:?}")
        }
    }
}
