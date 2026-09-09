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
//! Node-API surface for the Stillwind compiler.

mod api;
mod output;
mod session;

pub use api::*;
pub use session::NativeSessionError;
