//! Preserved CSS notices in published stylesheets.
//!
//! A published stylesheet starts with the Stillwind notice, the Tailwind notice, and
//! the notices that the selected definitions declare with `@stillwind-banner`.
//! [`MappedStylesheet::banners`](crate::MappedStylesheet::banners) records the notices
//! that each output range requires until the body is published.
//!
//! Call [`finalize_stylesheet`] to publish a complete stylesheet. A host that publishes
//! bodies itself calls [`stylesheet_banners`] and adds the returned notices.
//!
//! Modules:
//!
//! - [`finalize`] orders the notices and adds them before a body.
//! - [`tailwind`] holds the Tailwind notice, which `tools/tailwind/banner.ts` generates
//!   from the installed Tailwind compiler.

mod finalize;
mod tailwind;

pub use finalize::{finalize_stylesheet, stylesheet_banners};
