//! Snapshot ownership and caches shared by editor queries.

use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_design_system::{DesignSystemRevision, DesignSystemSnapshot};
use stillwind_diagnostics::LintSettings;

use super::canonical::CanonicalIndex;
use super::context::QueryContext;
use super::ordering::CandidateOrdering;
use crate::CompilerOptions;

/// Answers editor queries against one immutable snapshot.
///
/// Holding the snapshot rather than a project keeps editor answers independent of
/// module state and deterministic for as long as the revision does not change.
#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    /// Snapshot and resolved compiler settings shared by candidate operations.
    pub(super) context: QueryContext,
    /// Lazily retained index for this snapshot and its canonical conversion policy.
    pub(super) canonical_index: OnceCell<Arc<CanonicalIndex>>,
    /// Canonical replacements keyed by full candidate spelling. `None` records no
    /// verified replacement.
    pub(super) canonical_cache: RefCell<BTreeMap<LeanStr, Option<LeanStr>>>,
    /// Ordering keys and unresolved spellings reused by comparisons and batch ranking.
    pub(super) ordering: CandidateOrdering,
    /// Variant spellings with empty `[]` captures and no trailing colon, mapped to
    /// filled probes valid in element context. Completion validates each probe again
    /// under the requested input role.
    pub(super) variant_placeholders: OnceCell<BTreeMap<LeanStr, LeanStr>>,
}

impl DesignSystemQuery {
    /// Creates a query pinned to `design_system`.
    ///
    /// The options' prefixes are resolved against the snapshot so rendered previews
    /// match what a compilation would emit. Canonical queries prepare indexes when they
    /// need them and share that preparation with other queries for the same snapshot
    /// and spacing conversion policy.
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let context = QueryContext::new(design_system, options);
        Self {
            context,
            canonical_index: OnceCell::new(),
            canonical_cache: RefCell::new(BTreeMap::new()),
            ordering: CandidateOrdering::default(),
            variant_placeholders: OnceCell::new(),
        }
    }

    /// Returns the immutable design-system revision used by every query answer.
    pub fn revision(&self) -> DesignSystemRevision {
        self.context.design_system.revision()
    }

    /// Returns effective lint settings after compiler defaults and project settings.
    pub fn lint_settings(&self) -> LintSettings {
        self.context.options.lint_settings()
    }

    /// Returns the retained canonical spelling index, preparing it on first use.
    pub(super) fn canonical_index(&self) -> &Arc<CanonicalIndex> {
        self.canonical_index
            .get_or_init(|| self.context.canonical_index())
    }
}

/// Prepares canonical spelling indexes for `design_system` and effective `options`.
/// CSS loaders can call this before publishing a snapshot. Canonical queries use
/// the same cache, including when a caller skips this step. Malformed utility
/// requirements remain deferred until a matching candidate is evaluated.
pub fn prepare_canonical_index(design_system: &DesignSystemSnapshot, options: &CompilerOptions) {
    QueryContext::new(design_system.clone(), options.clone()).canonical_index();
}
