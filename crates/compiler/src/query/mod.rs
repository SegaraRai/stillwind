//! Editor queries resolve candidates against one immutable design-system snapshot.
//!
//! [`DesignSystemQuery`] pins each answer to a revision so hosts can discard stale
//! results. [`resolution`] generates candidate previews and diagnostics; [`ordering`]
//! compares candidates using compiler order. [`completion`] offers utility, variant,
//! and theme labels, and [`canonical`] finds equivalent spellings under the active lint
//! policy. [`theme`] exposes theme values and resolves colors for editor decorations.
//! [`context`] evaluates CSS and retrieves shared canonical indexes for queries and
//! [`prepare_canonical_index`], which loaders can use before exposing a revision.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::LintSettings;

use crate::CompilerOptions;

mod canonical;
mod completion;
mod context;
mod ordering;
mod resolution;
mod theme;

pub use completion::{CompletionInput, CompletionItem};
pub use resolution::{CandidateHover, CandidateResolution};

use canonical::CanonicalIndex;
use context::QueryContext;
use ordering::CandidateOrdering;

/// Answers candidate questions against one immutable snapshot.
///
/// Holding the snapshot rather than a project keeps editor answers independent of
/// module state and deterministic for as long as the revision does not change.
#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    context: QueryContext,
    canonical_index: Arc<CanonicalIndex>,
    canonical_cache: RefCell<BTreeMap<String, Option<String>>>,
    ordering: CandidateOrdering,
}

impl DesignSystemQuery {
    /// Creates a query pinned to `design_system`.
    ///
    /// The options' prefixes are resolved against the snapshot so rendered previews
    /// match what a compilation would emit. The first query for a snapshot and its
    /// spacing conversion policy prepares indexes; later queries share that preparation.
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let context = QueryContext::new(design_system, options);
        let canonical_index = context.canonical_index();
        Self {
            context,
            canonical_index,
            canonical_cache: RefCell::new(BTreeMap::new()),
            ordering: CandidateOrdering::default(),
        }
    }

    /// Returns effective lint settings after stylesheet severity and project overrides.
    pub fn lint_settings(&self) -> LintSettings {
        self.context
            .options
            .lint_settings(&self.context.design_system)
    }
}

/// Prepares canonical spelling indexes for `design_system` and effective `options`.
/// CSS loaders can call this before publishing a snapshot. Query construction uses
/// the same cache, including when a caller skips this step. Malformed utility
/// requirements remain deferred until a matching candidate is evaluated.
pub fn prepare_canonical_index(design_system: &DesignSystemSnapshot, options: &CompilerOptions) {
    QueryContext::new(design_system.clone(), options.clone()).canonical_index();
}

/// Finds a delimiter outside arbitrary values, selectors, functions, and escaped text.
fn last_top_level(value: &str, delimiter: char) -> Option<usize> {
    let mut depth = 0_usize;
    let mut escaped = false;
    let mut result = None;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            character if character == delimiter && depth == 0 => result = Some(index),
            _ => {}
        }
    }
    result
}
