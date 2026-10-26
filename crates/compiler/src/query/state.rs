//! Snapshot ownership and caches shared by editor queries.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lean_string::LeanStr;

use stillwind_design_system::{DesignSystemRevision, DesignSystemSnapshot};
use stillwind_diagnostics::LintSettings;
use stillwind_source::Fingerprint;

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
    /// The cache of canonical spellings. `shared` sets it to the cache of the snapshot.
    /// Otherwise the query creates its own empty cache on first use.
    pub(super) canonical_cache: OnceCell<Arc<CanonicalCache>>,
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
            canonical_cache: OnceCell::new(),
            ordering: CandidateOrdering::default(),
            variant_placeholders: OnceCell::new(),
        }
    }

    /// Creates a query like `new`, but with a cache of canonical spellings that it
    /// shares. Every query that this function creates for the same snapshot and the
    /// same resolved options uses the same cache.
    ///
    /// Compilation creates one query for each module. If each query had its own cache,
    /// the compiler would compute the same canonical spellings again for every module
    /// and after every edit.
    pub(crate) fn shared(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let query = Self::new(design_system, options);
        let cache = query.context.design_system.analysis(
            Fingerprint::of(format!(
                "canonical-candidates/v1\0{:?}",
                query.context.options
            )),
            CanonicalCache::default,
        );
        let _ = query.canonical_cache.set(cache);
        query
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

    /// Returns the cache of canonical spellings that this query reads and writes.
    pub(super) fn canonical_cache(&self) -> &Arc<CanonicalCache> {
        self.canonical_cache.get_or_init(Arc::default)
    }
}

/// Canonical spellings that were already computed for one snapshot and one set of
/// resolved options. The cache does not store results that ended with an error about
/// malformed requirements.
#[derive(Debug, Default)]
pub(super) struct CanonicalCache {
    /// Results of `canonical_candidate`, keyed by the complete candidate. `None` means
    /// that the candidate has no verified replacement.
    pub(super) candidates: Mutex<BTreeMap<LeanStr, Option<LeanStr>>>,
    /// Results of `canonical_candidates`, keyed by the input candidates in source
    /// order.
    pub(super) sets: Mutex<BTreeMap<Vec<LeanStr>, Vec<LeanStr>>>,
}

/// Prepares canonical spelling indexes for `design_system` and effective `options`.
/// CSS loaders can call this before publishing a snapshot. Canonical queries use
/// the same cache, including when a caller skips this step. Malformed utility
/// requirements remain deferred until a matching candidate is evaluated.
pub fn prepare_canonical_index(design_system: &DesignSystemSnapshot, options: &CompilerOptions) {
    QueryContext::new(design_system.clone(), options.clone()).canonical_index();
}
