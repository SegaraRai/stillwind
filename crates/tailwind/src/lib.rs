//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod builtins;
mod syntax;
mod utilities;
mod values;
mod variants;

pub use builtins::*;
pub use utilities::*;
pub use variants::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;
