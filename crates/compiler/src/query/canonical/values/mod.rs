//! Indexes canonical candidates derived from theme values.
//!
//! Modules:
//!
//! - [`direct`] shares value lookups between families that copy theme values.
//! - [`index`] prepares CSS templates and matches their normalized output.

mod direct;
mod index;

pub(super) use index::ThemeValueIndex;
