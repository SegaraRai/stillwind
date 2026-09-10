//! CSS syntax and value primitives, custom properties, resources, and output templates.

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

pub mod blocks;
mod colors;
mod custom_properties;
pub mod functions;
mod math;
mod output;
mod resources;
mod values;

pub use colors::*;
pub use custom_properties::*;
pub use math::*;
pub use output::*;
pub use resources::*;
pub use values::*;

#[cfg(test)]
mod tests;
