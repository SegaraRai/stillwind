//! Source buffers, spans, line indexes, and diagnostics used by every compiler pass.

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

mod diagnostics;
mod lint;
mod text;

pub use diagnostics::*;
pub use lint::*;
pub use text::*;
