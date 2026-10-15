//! Fatal diagnostic payloads, reporting identities, and source highlights.
//!
//! `diagnostic` owns payloads and message templates. `code` assigns unique reporting
//! identities and their classes. `source_failure` preserves coordinates when converting
//! source failures.
//! `finding` attaches highlights and notes through `CompileError::new`.

mod code;
mod diagnostic;
mod finding;
mod source_failure;

pub use code::{CompileErrorClass, CompileErrorCode};
pub use diagnostic::{CheckerFactsSubject, CompileDiagnostic};
pub use finding::CompileError;
pub use source_failure::SourceFailureContext;
