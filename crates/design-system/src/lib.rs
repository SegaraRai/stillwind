//! CSS-defined styling semantics, caller-driven loading, and immutable snapshots.

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

mod evaluation;
mod loading;
mod options;
mod output;
mod registry;
mod snapshot;
mod theme;

pub use evaluation::*;
pub use loading::*;
pub use options::*;
pub use registry::*;
pub use snapshot::*;
pub use theme::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;
