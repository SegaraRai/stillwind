//! CSS parsing primitives, value validation, math and color handling, custom
//! properties, resource discovery, and output-template data.
//!
//! Everything here is either standard CSS or a lexical helper the design
//! system and the compiler build on; the crate deliberately knows nothing
//! about Stillwind's theme, registry or utility model. Property and value
//! grammar is delegated to Lightning CSS and to the generated Webref table, so
//! "is this a valid value for that property" is answered from published CSS
//! data rather than from a hand-maintained list.
//!
//! Boundaries worth knowing: `resources` finds URLs but never resolves them,
//! `blocks` scans raw text without understanding declarations, and `output`
//! describes where authored CSS goes without choosing the host's module or
//! chunk layout.

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
mod declarations;
mod functions;
mod math;
mod output;
mod resources;
mod selectors;
mod values;

pub use blocks::*;
pub use colors::*;
pub use custom_properties::*;
pub use declarations::*;
pub use functions::*;
pub use math::*;
pub use output::*;
pub use resources::*;
pub use selectors::*;
pub use values::*;
