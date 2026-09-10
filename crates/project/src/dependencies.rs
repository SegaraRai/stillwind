use std::collections::{BTreeMap, BTreeSet};

use stillwind_compiler::CompilationDependencies;
use stillwind_semantic::{IdentityId, ModuleId};

/// Reverse memberships derived from the currently published artifacts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyIndex {
    pub candidates: BTreeMap<String, BTreeSet<ModuleId>>,
    pub theme_tokens: BTreeMap<String, BTreeSet<ModuleId>>,
    pub utilities: BTreeMap<String, BTreeSet<ModuleId>>,
    pub variants: BTreeMap<String, BTreeSet<ModuleId>>,
    pub properties: BTreeMap<String, BTreeSet<ModuleId>>,
    pub assets: BTreeMap<IdentityId, BTreeSet<ModuleId>>,
}

impl DependencyIndex {
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
            (&next.utilities, &mut self.utilities),
            (&next.variants, &mut self.variants),
            (&next.properties, &mut self.properties),
        ] {
            insert_memberships(index, entries, module);
        }
        insert_memberships(&mut self.assets, &next.assets, module);
    }

    pub(crate) fn remove(&mut self, module: &ModuleId, previous: &CompilationDependencies) {
        for (entries, index) in [
            (&previous.candidates, &mut self.candidates),
            (&previous.theme_tokens, &mut self.theme_tokens),
            (&previous.utilities, &mut self.utilities),
            (&previous.variants, &mut self.variants),
            (&previous.properties, &mut self.properties),
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
