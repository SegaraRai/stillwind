//! Fatal diagnostic payloads, reporting identities, and source highlights.
//!
//! Modules:
//!
//! - [`code`] assigns unique reporting identities and their classes.
//! - [`diagnostic`] owns payloads and message templates.
//! - [`finding`] attaches highlights and notes through [`CompileError::new`].
//! - [`source_failure`] keeps coordinates when it converts source failures.

mod code;
mod diagnostic;
mod finding;
mod source_failure;

pub use code::{CompileErrorClass, CompileErrorCode};
pub use diagnostic::{CheckerFactsSubject, CompileDiagnostic};
pub use finding::CompileError;
pub use source_failure::SourceFailureContext;
