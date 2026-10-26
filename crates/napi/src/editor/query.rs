//! Defines the editor query type. The other editor modules add its requests.

use std::fmt::Display;

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{CompilerOptions, DesignSystemQuery};
use stillwind_design_system::DesignSystemSnapshot;

/// Answers editor requests for one design system that does not change. Requests that
/// need source text receive it as an argument.
#[napi]
pub struct NativeEditorQuery {
    pub(super) query: DesignSystemQuery,
}

impl NativeEditorQuery {
    /// Creates a query for the completed design system `snapshot`. `options` must be
    /// the compiler options of the loader that built `snapshot`.
    pub(crate) fn new(snapshot: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        Self {
            query: DesignSystemQuery::new(snapshot, options),
        }
    }
}

#[napi]
impl NativeEditorQuery {
    /// Returns the fingerprint of the design system. Completion results carry this
    /// value, so a client can detect that a result belongs to an older design system.
    #[napi]
    pub fn revision(&self) -> String {
        self.query.revision().0.to_string()
    }
}

/// Converts `error` into a JavaScript error with the same message.
pub(super) fn reason(error: impl Display) -> Error {
    Error::from_reason(error.to_string())
}
