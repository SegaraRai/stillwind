//! Completes partial candidates in the editor from the registry and theme of the
//! snapshot.
//!
//! Modules:
//!
//! - [`candidates`] completes one partial candidate for an input role.
//! - [`detail`] summarizes the CSS of a selected label.
//! - [`vocabulary`] lists the utility and variant labels that [`candidates`] filters.

mod candidates;
mod detail;
mod vocabulary;

pub use candidates::{CompletionInput, CompletionItem};
pub use detail::completion_detail;

#[cfg(test)]
mod tests;
