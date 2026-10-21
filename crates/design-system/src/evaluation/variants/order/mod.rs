//! Variant ordering, as declared by the registry rather than by markup.
//!
//! Each `@stillwind-variant` definition carries an authored position, and variants
//! sharing a sort group are ordered by the group's first position so that related
//! variants (for example a set of breakpoints) sort as one unit. A variant that no
//! definition matches sorts last, which keeps an unknown spelling from silently
//! acquiring the position of something it resembles.
//!
//! Within a group, a variant's sort key is the value of a theme token; numeric values
//! compare by unit and then magnitude, with direction applied to magnitude. Anything
//! that is not a number falls back to text order so the comparison stays total and
//! deterministic.
//!
//! [`sequence`] compares variant stacks and prepares their comparison keys,
//! [`position`] finds registry positions and sort groups, and [`sort_value`] compares
//! variants within one group.

mod position;
mod sequence;
mod sort_value;

pub use position::compound_variant_inner;
pub use sequence::{VariantSequenceOrder, compare_variant_sequences};

pub(in crate::evaluation) use position::variant_sort_namespace;
