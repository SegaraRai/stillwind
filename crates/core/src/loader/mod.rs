//! Caller-driven CSS loading. Source identities are deliberately opaque.

mod expansion;
mod imports;
mod protocol;
mod state;

pub use imports::CssImportError;
pub use protocol::*;
pub use state::{DesignSystemLoadError, DesignSystemLoader};

#[cfg(test)]
mod tests;
