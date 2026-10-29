//! Preserved CSS notices in published stylesheets.
//!
//! A published stylesheet starts with the Stillwind notice, the Tailwind notice, and
//! the notices that the selected definitions declare with `@stillwind-banner`.
//! [`MappedStylesheet::banners`](crate::MappedStylesheet::banners) records the notices
//! that each output range requires until the body is published.
//!
//! [`finalize`] orders the notices and adds them before a body. [`tailwind`] holds the
//! Tailwind notice, which `tools/tailwind/banner.ts` generates from the installed
//! Tailwind compiler.
//!
//! Call [`finalize_stylesheet`] when publishing a complete stylesheet. Hosts that
//! publish bodies themselves call [`stylesheet_banners`] and add the returned notices.

mod finalize;
mod tailwind;

pub use finalize::{finalize_stylesheet, stylesheet_banners};
