//! Contextual folding of ordered style contributions.
//!
//! [`expression`] retains property identities, control flow, and source occurrences.
//! [`fold`] checks field layout for a consumer; [`render`]
//! materializes the selected plan with its source mappings and helper bindings. `tests`
//! checks folding proofs and differential execution against the fragment runtime.
//!
//! Call [`render_style_call`] with finalized class allocation and a framework spec.
//! Inputs must satisfy the macro purity contract before output selection begins.
//! CSS allocation and frontend contract validation belong to the caller.

mod expression;
mod fold;
mod render;

pub(super) use expression::{Emitted, EmittedEntry, Property, ValueExpr};
pub(super) use render::render_style_call;

#[cfg(test)]
mod tests;
