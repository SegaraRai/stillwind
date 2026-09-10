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
mod options;
mod output;
mod semantic;
mod session;
mod source_input;
mod stylesheet;

pub use api::*;
pub use output::{BindingOutput, ProjectBindingOutput, ProjectModuleOutput};
pub use session::NativeSessionError;
pub use source_input::ProjectSource;
