//! Evaluates candidate CSS against resolved settings without requiring an editor query.
//!
//! Canonical preparation and query evaluation use this context. Preparation stores only
//! derived values in the snapshot, so no cached index retains its owning snapshot.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanString};

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
    pub(super) canonical_previews: RefCell<BTreeMap<LeanStr, Option<Arc<CanonicalPreview>>>>,
}

impl QueryContext {
    pub(super) fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        let canonical_policy = match options
            .lint_settings()
            .get(LintKind::PreferCanonicalUtilities)
        {
            Some(LintConfig::PreferCanonicalUtilities(config)) => config.clone(),
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
        let key = Fingerprint::of(
            format_args!(
                "canonical-index/v2\0{}\0{}\0{}\0{}\0{}",
                self.canonical_policy.px_to_rem,
                self.options.root_font_size.get().to_bits(),
                self.options.prefix,
                self.options.tailwind_prefix,
                self.canonical_policy.preserve_specificity
            )
            .to_lean_string(),
        );
        self.design_system.analysis(key, || {
            let index = CanonicalIndex::prepare(self);
            self.canonical_previews.borrow_mut().clear();
            index
        })
    }

    pub(super) fn canonical_options(&self) -> CanonicalUtilitiesConfig {
        self.canonical_policy.clone()
    }

    /// Returns the resolved inputs of this context without its preview cache.
    pub(super) fn inputs(&self) -> QueryInputs {
        QueryInputs {
            design_system: self.design_system.clone(),
            options: self.options.clone(),
            canonical_policy: self.canonical_policy.clone(),
        }
    }
}

/// Resolved inputs of a [`QueryContext`], which threads can share.
///
/// A [`QueryContext`] cannot be shared between threads because its preview cache is
/// not thread-safe. Each thread creates its own context from these inputs instead.
pub(super) struct QueryInputs {
    design_system: DesignSystemSnapshot,
    options: CompilerOptions,
    canonical_policy: CanonicalUtilitiesConfig,
}

impl QueryInputs {
    /// Returns a context with these inputs and an empty preview cache.
    pub(super) fn context(&self) -> QueryContext {
        QueryContext {
            design_system: self.design_system.clone(),
            options: self.options.clone(),
            canonical_policy: self.canonical_policy.clone(),
            canonical_previews: RefCell::new(BTreeMap::new()),
        }
    }
}
