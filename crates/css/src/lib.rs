//! CSS theme parsing, definition registration, and value validation.

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

mod blocks;
mod builtins;
mod colors;
mod custom_properties;
mod directives;
mod functions;
mod math;
mod registry;
mod theme;
mod values;

pub use builtins::*;
pub use colors::*;
pub use custom_properties::*;
pub use math::*;
pub use registry::*;
pub use theme::*;
pub use values::*;

#[cfg(test)]
mod tests;
