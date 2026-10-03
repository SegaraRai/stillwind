//! Reverse dependency index over the currently published artifacts.
//!
//! Hosts use it to answer "which modules must be recompiled when this theme token,
//! utility, variant, property, candidate, or asset changes". Entries are derived from
//! [`CompilationDependencies`] and describe the last published artifact.
//!
//! Entries reflect the compiler's recorded sets, not a proof of exact use. Candidates
//! are syntactic mentions, tokens include static and transitive dependencies, and
//! assets include declarations as well as external facts. Utilities, variants, and
//! properties are especially conservative: the compiler records every definition in
//! those three maps, because a candidate's generated content depends on which
//! definition it resolves against and that cannot be narrowed without tracking usage
//! during resolution. A change to an already indexed definition therefore includes
//! every module. New names have no reverse entry; hosts must handle additions
//! separately. Session reloads currently rebuild all retained analyses instead of
//! selecting through this index.
//!
//! The index is not append-only. A module that stops using a definition is removed from
//! that definition's entry, and an entry disappears with its last member.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_compiler::CompilationDependencies;
use stillwind_semantic::IdentityId;
use stillwind_source::ModuleId;

/// Reverse memberships derived from the currently published artifacts.
///
/// Every map is keyed by the definition's textual or identity form and holds the
/// modules that referenced it. Keys disappear when their last member is removed, so an
/// index with no entries means no module depends on anything.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyIndex {
    /// Candidate text to the modules that evaluate it.
    pub candidates: BTreeMap<String, BTreeSet<ModuleId>>,
    /// Theme token name to the modules that reference it.
    pub theme_tokens: BTreeMap<String, BTreeSet<ModuleId>>,
    /// Utility definition name to the modules conservatively depending on it.
    pub utilities: BTreeMap<String, BTreeSet<ModuleId>>,
    /// Variant definition name to the modules conservatively depending on it.
    pub variants: BTreeMap<String, BTreeSet<ModuleId>>,
    /// Property definition name to the modules conservatively depending on it.
    pub properties: BTreeMap<String, BTreeSet<ModuleId>>,
    /// Asset identity to modules declaring it or receiving it in external facts.
    pub assets: BTreeMap<IdentityId, BTreeSet<ModuleId>>,
}

impl DependencyIndex {
    /// Swaps one module's entries, retracting the previous set first.
    ///
    /// The previous value must be the one that produced the current entries; a `None`
    /// previous leaves old entries in place, so callers that lost the artifact must
    /// call [`DependencyIndex::remove`] before replacing.
    pub(crate) fn replace(
        &mut self,
        module: &ModuleId,
        previous: Option<&CompilationDependencies>,
        next: &CompilationDependencies,
    ) {
        if let Some(previous) = previous {
            self.remove(module, previous);
        }
        for (entries, index) in [
            (&next.candidates, &mut self.candidates),
            (&next.theme_tokens, &mut self.theme_tokens),
            (next.utilities.as_ref(), &mut self.utilities),
            (next.variants.as_ref(), &mut self.variants),
            (next.properties.as_ref(), &mut self.properties),
        ] {
            insert_memberships(index, entries, module);
        }
        insert_memberships(&mut self.assets, &next.assets, module);
    }

    pub(crate) fn remove(&mut self, module: &ModuleId, previous: &CompilationDependencies) {
        for (entries, index) in [
            (&previous.candidates, &mut self.candidates),
            (&previous.theme_tokens, &mut self.theme_tokens),
            (previous.utilities.as_ref(), &mut self.utilities),
            (previous.variants.as_ref(), &mut self.variants),
            (previous.properties.as_ref(), &mut self.properties),
        ] {
            remove_memberships(index, entries, module);
        }
        remove_memberships(&mut self.assets, &previous.assets, module);
    }
}

fn insert_memberships<K: Ord + Clone>(
    index: &mut BTreeMap<K, BTreeSet<ModuleId>>,
    entries: &BTreeSet<K>,
    module: &ModuleId,
) {
    for entry in entries {
        index
            .entry(entry.clone())
            .or_default()
            .insert(module.clone());
    }
}

fn remove_memberships<K: Ord>(
    index: &mut BTreeMap<K, BTreeSet<ModuleId>>,
    entries: &BTreeSet<K>,
    module: &ModuleId,
) {
    for entry in entries {
        if let Some(users) = index.get_mut(entry) {
            users.remove(module);
            if users.is_empty() {
                index.remove(entry);
            }
        }
    }
}
