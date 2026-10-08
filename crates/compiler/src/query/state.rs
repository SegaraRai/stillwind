//! Snapshot ownership and caches shared by editor queries.

use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::LintSettings;

use super::canonical::CanonicalIndex;
use super::context::QueryContext;
use super::ordering::CandidateOrdering;
use crate::CompilerOptions;

/// Answers candidate questions against one immutable snapshot.
///
/// Holding the snapshot rather than a project keeps editor answers independent of
/// module state and deterministic for as long as the revision does not change.
#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    pub(super) context: QueryContext,
    pub(super) canonical_index: OnceCell<Arc<CanonicalIndex>>,
    pub(super) canonical_cache: RefCell<BTreeMap<String, Option<String>>>,
    pub(super) ordering: CandidateOrdering,
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
        }
    }

    /// Returns effective lint settings after compiler defaults and project settings.
    pub fn lint_settings(&self) -> LintSettings {
        self.context.options.lint_settings()
    }

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
