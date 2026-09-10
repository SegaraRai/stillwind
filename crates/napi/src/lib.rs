//! Node-API surface for the Stillwind compiler.

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

mod api;
mod output;
mod session;
mod source_input;

pub use api::*;
pub use session::NativeSessionError;
