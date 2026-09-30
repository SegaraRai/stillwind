//! Ref extraction: definitions, references, projections, validation.
//!
//! [`definitions`] validates `style.ref()` calls and the ref variant forms,
//! [`references`] resolves uses back to declarations or checker facts, [`projections`]
//! lowers `asAnchor` / `positionAnchor` / `asContainer` / `at` uses and scope lists,
//! and [`validation`] checks providers, attachments, and JSX ancestry within this
//! module.
//!
//! Only the pieces other modules need are re-exported.

mod definitions;
mod projections;
mod references;
mod validation;

pub(super) use projections::*;
