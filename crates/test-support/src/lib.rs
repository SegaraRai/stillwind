//! Shared CSS fixtures and assertions for workspace tests.
//!
//! `css` constructs source buffers and expands imports from `packages/core`.
//! `defaults` provides the package CSS in package order.
//! `assertions` compares CSS tokens after arithmetic evaluation.
//! `fixtures` reads Markdown cases and catalogs.
//! `tailwind` maps upstream fixture imports to the standard package.
//!
//! Test targets use this crate through dev-dependencies.

mod assertions;
mod css;
mod defaults;
mod fixtures;
mod tailwind;

pub use assertions::{CssAssertions, css_tokens};
pub use css::{expand as expand_css, source as css_source, with_defaults};
pub use defaults::PACKAGE_CSS;
pub use fixtures::{candidates, case_for_candidate, case_named, catalog_section, code_block};
pub use tailwind::fixture_css;
