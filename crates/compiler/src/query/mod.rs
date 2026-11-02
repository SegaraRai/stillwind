//! Editor queries that resolve candidates against one immutable design-system snapshot.
//!
//! [`DesignSystemQuery`] pins each answer to a revision, so hosts can discard stale
//! results.
//!
//! Modules:
//!
//! - [`annotations`] adds comments with resolved values to preview CSS.
//! - [`canonical`] finds equivalent spellings under the active lint policy.
//! - [`completion`] offers utility, variant, marker, and theme labels.
//! - [`context`] evaluates CSS and provides the shared canonical indexes, including
//!   [`prepare_canonical_index`], which a loader can call before it exposes a revision.
//! - [`input`] selects the compiler resolver for each source input role.
//! - [`markers`] describes the marker class that a candidate adds.
//! - [`ordering`] compares candidates in compiler order.
//! - [`resolution`] generates candidate previews and diagnostics.
//! - [`state`] owns the pinned snapshot and caches.
//! - [`stylesheet`] answers completion, hover, color, and sorting requests for authored
//!   CSS, including incomplete CSS.
//! - [`theme`] exposes theme values and resolves colors for editor decorations.

mod annotations;
mod canonical;
mod completion;
mod context;
mod input;
mod markers;
mod ordering;
mod resolution;
mod state;
mod stylesheet;
mod theme;

pub use annotations::{ColorEquivalents, PreviewAnnotations};
pub use completion::{CompletionInput, CompletionItem, completion_detail};
pub use input::CandidateContext;
pub use markers::AddedMarker;
pub use resolution::{CandidateFailure, CandidateHover, CandidateResolution};
pub use state::{DesignSystemQuery, prepare_canonical_index};
pub use stylesheet::{
    StylesheetColor, StylesheetCompletion, StylesheetCompletionOptions, StylesheetEdit,
    StylesheetHover, StylesheetPluginPath, StylesheetSortOptions, editor_css_source,
    stylesheet_plugin_paths,
};
pub use theme::SchemeColor;

use context::QueryContext;
