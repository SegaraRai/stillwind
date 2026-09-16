//! Ref and subtree extraction: definitions, references, projections, validation.
//!
//! [`definitions`] validates `style.ref()` / `style.subtree()` calls and the ref variant
//! forms, [`references`] resolves uses back to declarations or checker facts,
//! [`projections`] lowers `anchor` / `position` / `subtree` / `container` / `at`
//! projections, and [`validation`] checks the JSX ancestry rules that can be decided
//! within one file.
//!
//! Only the pieces other modules need are re-exported.

mod definitions;
mod projections;
mod references;
mod validation;

pub(super) use projections::*;
