//! Language-neutral intermediate representation shared by all source frontends.

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

mod analysis;
mod composition;
mod framework;
mod input;
mod model;
mod spans;

pub use analysis::*;
pub use composition::*;
pub use framework::*;
pub use input::*;
pub use model::*;
