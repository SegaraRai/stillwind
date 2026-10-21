//! Caller-driven CSS loading. Source identities are deliberately opaque.
//!
//! The loader turns host-supplied buffers into one snapshot. It parses imports in
//! authored order, reports every outstanding dependency as a
//! [`protocol::CssFileRequest`], and never performs I/O, path resolution, or specifier
//! resolution itself: the host decides what `(importer, specifier)` means and which
//! name to supply next.
//!
//! Request IDs start at 1 and increase monotonically for the life of one loader; the
//! initial batch is a single root source, conventionally ID 0. Unknown or duplicate
//! response IDs, and a repeated name with different content, are protocol errors.
//! Completion is terminal, and dropping an incomplete loader is the entire cancellation
//! mechanism.
//!
//! `imports` parses dependency requests and their conditions, and records the Tailwind
//! features that loading ignores. `expansion` combines the supplied sources while
//! retaining origins. `protocol` defines requests, responses, and source graphs.
//! `state` owns request tracking and snapshot completion.

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
