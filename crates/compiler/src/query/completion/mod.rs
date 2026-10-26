//! Completes partial candidates in the editor, using the registry and theme of the
//! snapshot.
//!
//! [`candidates`] completes one partial candidate for an input role. [`vocabulary`]
//! lists the utility and variant labels that [`candidates`] filters. [`detail`]
//! summarizes the CSS of a selected label.

mod candidates;
mod detail;
mod vocabulary;

pub use candidates::{CompletionInput, CompletionItem};
pub use detail::completion_detail;

#[cfg(test)]
mod tests;
