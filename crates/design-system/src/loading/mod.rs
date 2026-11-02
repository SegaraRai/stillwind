//! Caller-driven CSS loading with opaque source identities.
//!
//! The loader turns host-supplied buffers into one snapshot. It parses imports in
//! authored order and reports every outstanding dependency as a
//! [`protocol::CssFileRequest`]. It never performs I/O, path resolution, or specifier
//! resolution itself: the host decides what `(importer, specifier)` means and which
//! name to supply next.
//!
//! Request IDs start at 1 and increase for the life of one loader. The initial batch is
//! a single root source, conventionally ID 0. Unknown or duplicate response IDs, and a
//! repeated name with different content, are protocol errors. Completion is final, and
//! dropping an incomplete loader is the only way to cancel loading.
//!
//! Modules:
//!
//! - [`expansion`] combines the supplied sources and keeps their origins.
//! - [`imports`] parses dependency requests and their conditions, and records the
//!   Tailwind features that loading ignores.
//! - [`protocol`] defines requests, responses, and source graphs.
//! - [`state`] tracks requests and completes the snapshot.

mod expansion;
mod imports;
mod protocol;
mod state;

pub use imports::{IgnoredTailwindFeature, IgnoredTailwindFeatureKind, ignored_tailwind_features};
pub use protocol::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssResourceReference, CssSource,
    CssSourceEdge, CssSourceGraph, CssSourceMapping, DesignSystemLoadStep,
};
pub use state::{DesignSystemLoadError, DesignSystemLoader};

#[cfg(test)]
mod tests;
