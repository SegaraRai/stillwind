//! Evaluates candidate CSS against resolved settings without requiring an editor query.
//!
//! Canonical preparation and query evaluation use this context. Preparation stores only
//! derived values in the snapshot, so no cached index retains its owning snapshot.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CanonicalUtilitiesConfig, LintConfig, LintKind};
use stillwind_source::Fingerprint;

use super::canonical::{CanonicalIndex, CanonicalPreview};
use crate::CompilerOptions;

/// Resolved inputs and temporary candidate previews for one evaluation scope.
#[derive(Clone, Debug)]
pub(super) struct QueryContext {
    /// Immutable definitions and sources used by this evaluation.
    pub(super) design_system: DesignSystemSnapshot,
    /// Compiler options using the snapshot's prefixes and utility ordering.
    pub(super) options: CompilerOptions,
    /// Conversion policy after applying project lint overrides to compiler defaults.
    pub(super) canonical_policy: CanonicalUtilitiesConfig,
    /// Candidate CSS and signatures scoped to the resolved inputs above.
    pub(super) canonical_previews: RefCell<BTreeMap<String, Option<Arc<CanonicalPreview>>>>,
}

impl QueryContext {
    pub(super) fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        let canonical_policy = match options.lint_settings().get(LintKind::CanonicalUtilities) {
            Some(LintConfig::CanonicalUtilities(config)) => config.clone(),
            _ => CanonicalUtilitiesConfig::default(),
        };
        Self {
            design_system,
            options,
            canonical_policy,
            canonical_previews: RefCell::new(BTreeMap::new()),
        }
    }

    /// Retrieves or prepares the index for this snapshot and its effective compiler settings.
    /// Temporary previews used during preparation are released after the index owns
    /// the values it needs. Requirement failures remain deferred until candidate use.
    pub(super) fn canonical_index(&self) -> Arc<CanonicalIndex> {
        let key = Fingerprint::of(format!(
            "canonical-index/v1\0{}\0{}\0{}\0{}",
            self.canonical_policy.px_to_rem,
            self.canonical_policy.root_font_size.get().to_bits(),
            self.options.prefix,
            self.options.tailwind_prefix
        ));
        self.design_system.analysis(key, || {
            let index = CanonicalIndex::prepare(self);
            self.canonical_previews.borrow_mut().clear();
            index
        })
    }

    pub(super) fn canonical_options(&self) -> CanonicalUtilitiesConfig {
        self.canonical_policy.clone()
    }
}
