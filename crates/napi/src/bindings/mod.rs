//! Node-API surface for running the Stillwind compiler inside Vite.

mod api;
mod output;
mod session;

pub use api::*;
pub use session::NativeSessionError;
