//! Contextual folding of ordered style contributions.
//!
//! Call [`render_style_call`] with finalized class allocation and an output spec.
//! Inputs must satisfy the macro purity contract before output selection begins. The
//! caller owns CSS allocation and frontend contract validation.
//!
//! Modules:
//!
//! - [`expression`] keeps property identities, control flow, and source occurrences.
//! - [`fold`] checks the field layout for a consumer.
//! - [`render`] materializes the selected plan with its source mappings and helper
//!   bindings.

mod expression;
mod fold;
mod render;

pub(super) use expression::{Emitted, EmittedEntry, Property, ValueExpr};
pub(super) use render::{render_cva_value, render_style_call};

#[cfg(test)]
mod tests;
