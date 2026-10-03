//! Shared CSS fixtures and assertions for workspace tests.
//!
//! `css` constructs source buffers and expands imports from `packages/core`.
//! `defaults` provides the builtin CSS in package order.
//! `assertions` compares CSS tokens after arithmetic evaluation.
//! `fixtures` reads Markdown cases and catalogs.
//! `tailwind` maps upstream fixture imports to the standard package.
//!
//! Test targets use this crate through dev-dependencies.

pub mod assertions;
pub mod css;
pub mod defaults;
pub mod fixtures;
pub mod tailwind;
