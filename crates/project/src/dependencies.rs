//! Maps recorded definitions and assets to published modules.
//!
//! Hosts use [`DependencyIndex`] to find modules affected by a definition change.
//! Entries come from [`CompilationDependencies`] in the last published artifacts.
//!
//! Entries may include unused definitions. Candidates come from source syntax.
//! Tokens include direct and indirect dependencies. Assets include declarations
//! and external facts. The compiler records all utility, variant, and property names
//! because it does not track which definition each candidate resolves to.
//!
//! Changing an indexed utility, variant, or property therefore selects every module.
//! New names have no index entry, so hosts must handle additions separately.
//! Session reloads rebuild all retained analyses without using this index to select
//! modules.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;

use stillwind_compiler::CompilationDependencies;
use stillwind_semantic::IdentityId;
use stillwind_source::ModuleId;

/// Records which published modules depend on each definition or asset.
///
/// Each key is a definition name or asset identity. Its value contains module IDs.
/// Removing the last module from an entry also removes its key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyIndex {
    /// Maps candidate text to modules that mention it.
    pub candidates: BTreeMap<LeanStr, BTreeSet<ModuleId>>,
    /// Maps names of theme tokens and shared assets to modules that reference them.
    pub theme_values: BTreeMap<LeanStr, BTreeSet<ModuleId>>,
    /// Maps utility names to modules, including modules that do not use the utility.
    pub utilities: BTreeMap<LeanStr, BTreeSet<ModuleId>>,
    /// Maps variant names to modules, including modules that do not use the variant.
    pub variants: BTreeMap<LeanStr, BTreeSet<ModuleId>>,
    /// Maps property names to modules, including modules that do not use the property.
    pub properties: BTreeMap<LeanStr, BTreeSet<ModuleId>>,
    /// Maps asset identities to modules that declare them or receive them in external
    /// facts.
    pub assets: BTreeMap<IdentityId, BTreeSet<ModuleId>>,
}

impl DependencyIndex {
    /// Replaces dependency entries for `module` with those in `next`.
    ///
    /// `previous` must match the dependencies used to create the existing entries.
    /// These entries are removed before adding `next`. Passing `None` keeps old
    /// entries, so callers without the old artifact must call [`Self::remove`] first.
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
            (&next.theme_values, &mut self.theme_values),
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
            (&previous.theme_values, &mut self.theme_values),
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
