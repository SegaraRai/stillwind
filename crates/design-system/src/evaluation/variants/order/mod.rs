//! Variant order, as the registry declares it instead of as markup writes it.
//!
//! Each `@stillwind-variant` definition carries an authored position. Variants that
//! share a sort group are ordered by the first position of the group, so related
//! variants, such as a set of breakpoints, sort as one unit. A variant that no
//! definition matches sorts last, so an unknown spelling never takes the position of a
//! similar variant.
//!
//! Within a group, the sort key of a variant is the value of a theme token. Numeric
//! values compare by unit and then by magnitude, and the direction applies to the
//! magnitude. Other values fall back to text order, so the comparison stays total and
//! deterministic.
//!
//! Modules:
//!
//! - [`position`] finds registry positions and sort groups.
//! - [`sequence`] compares variant stacks and prepares their comparison keys.
//! - [`sort_value`] compares variants within one group.

mod position;
mod sequence;
mod sort_value;

pub use position::compound_variant_inner;
pub use sequence::{VariantSequenceOrder, compare_variant_sequences};

pub(in crate::evaluation) use position::variant_sort_namespace;
