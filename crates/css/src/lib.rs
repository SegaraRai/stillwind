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
mod colors;
mod custom_properties;
mod functions;
mod math;
mod output;
mod registry;
mod theme;
mod values;

pub use colors::*;
pub use custom_properties::*;
pub use math::*;
pub use output::*;
pub use registry::*;
pub use theme::*;
pub use values::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;
