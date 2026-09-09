//! Source buffers, spans, line indexes, and diagnostics used by every compiler pass.

mod diagnostics;
mod lint;
mod text;

#[cfg(test)]
mod tests;

pub use diagnostics::*;
pub use lint::*;
pub use text::*;
