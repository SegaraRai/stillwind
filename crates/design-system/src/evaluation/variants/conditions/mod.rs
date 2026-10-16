//! Relations between the conditions of a candidate's variants.
//!
//! [`redundant_variants`] finds variants that another variant of the same candidate
//! already requires, and [`unsatisfiable_variants`] proves that a variant stack can
//! never match. Both report only what they can prove; a condition that this module
//! cannot interpret is compared by its text.
//!
//! [`atoms`] defines simple conditions and turns resolved applications into branches of
//! them. [`queries`] reads media and container queries, and [`selectors`] reads
//! selector conditions on the styled element. [`stack`] resolves a candidate's variants
//! with their text ranges, [`redundancy`] finds redundant variants, and
//! [`satisfiability`] finds contradictions.

mod atoms;
mod queries;
mod redundancy;
mod satisfiability;
mod selectors;
mod stack;

pub use redundancy::{RedundantVariant, VariantRedundancy, redundant_variants};
pub use satisfiability::{UnsatisfiableVariants, unsatisfiable_variants};
