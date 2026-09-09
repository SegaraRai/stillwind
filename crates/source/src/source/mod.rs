//! Source buffers, spans, line indexes, and diagnostics used by every compiler pass.

use std::{ops::Range, sync::Arc};

use serde::{Deserialize, Serialize};

mod text;
pub use text::*;
mod diagnostics;
pub use diagnostics::*;
mod lint;
pub use lint::*;

#[cfg(test)]
mod tests;
