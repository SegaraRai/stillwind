//! Editor queries resolve candidates against one immutable design-system snapshot.
//!
//! [`DesignSystemQuery`] pins each answer to a revision so hosts can discard stale
//! results. [`resolution`] generates candidate previews and diagnostics. [`ordering`]
//! compares candidates using compiler order. [`completion`] offers utility, variant,
//! marker, and theme labels, and [`markers`] describes the marker class that a
//! candidate adds. [`canonical`] finds equivalent spellings under the active lint
//! policy. [`theme`] exposes theme values and resolves colors for editor decorations,
//! and [`annotations`] adds comments with resolved values to preview CSS.
//! [`stylesheet`] locates CSS inputs and reuses candidate queries for completion,
//! hover, colors, and sorting edits. It also recovers definitions from incomplete CSS
//! for completion, hover, and color queries. [`input`] selects the compiler resolver
//! for each source input role. [`context`] evaluates CSS and retrieves shared canonical
//! indexes for queries and [`prepare_canonical_index`], which loaders can use before
//! exposing a revision. [`state`] owns the pinned snapshot and caches. [`syntax`] scans
//! candidate delimiters.

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
mod syntax;
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

use context::QueryContext;
use syntax::last_top_level;
