//! Contextual folding of ordered style contributions.
//!
//! `expression` retains property identities, effect proofs, control flow, and source
//! occurrences. `fold` checks field layout and evaluation constraints for a consumer;
//! `render` materializes the selected plan with its source mappings and helper bindings.
//! `tests` checks folding proofs and differential execution against the fragment runtime.
//!
//! Call [`render_style_call`] with finalized class allocation and a framework spec.
//! It may report a root order obstruction as a lint while retaining runtime composition.
//! CSS allocation and frontend effect analysis belong to the caller.

pub(super) mod expression;
mod fold;
mod render;

pub(super) use render::render_style_call;

#[cfg(test)]
mod tests;
