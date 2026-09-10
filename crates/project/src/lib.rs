//! Mutable artifact ownership, reloads, and explicit stylesheet coverage and placement.

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

mod dependencies;
mod session;
mod stylesheet_plan;

pub use dependencies::DependencyIndex;
pub use session::*;
pub use stylesheet_plan::*;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;
