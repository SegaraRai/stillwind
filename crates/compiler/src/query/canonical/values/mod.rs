//! Indexes canonical candidates derived from theme values.
//!
//! `index` prepares CSS templates and matches their normalized output.
//! `direct` shares value lookups across families that copy theme values.

mod direct;
mod index;

pub(super) use index::ThemeValueIndex;
