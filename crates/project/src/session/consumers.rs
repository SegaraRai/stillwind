//! Indexes published composition edges by the module that supplies the base style.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_compiler::CompiledModule;
use stillwind_source::ModuleId;

/// Published consumers of each base module.
///
/// A consumer is a module whose published artifact has a composition edge to the base.
/// The update methods must receive the artifact that the session publishes or removes,
/// so the index matches the published artifacts.
#[derive(Clone, Debug, Default)]
pub(super) struct CompositionConsumers {
    consumers: BTreeMap<ModuleId, BTreeSet<ModuleId>>,
}

impl CompositionConsumers {
    /// Records the composition edges of the artifact published for `module`. The caller
    /// must first remove the artifact that the new artifact replaces.
    pub(super) fn insert(&mut self, module: &ModuleId, artifact: &CompiledModule) {
        for edge in &artifact.composition {
            self.consumers
                .entry(edge.base.module_id().clone())
                .or_default()
                .insert(module.clone());
        }
    }

    /// Removes the composition edges of the artifact removed for `module`.
    pub(super) fn remove(&mut self, module: &ModuleId, artifact: &CompiledModule) {
        for edge in &artifact.composition {
            let base = edge.base.module_id();
            if let Some(consumers) = self.consumers.get_mut(base) {
                consumers.remove(module);
                if consumers.is_empty() {
                    self.consumers.remove(base);
                }
            }
        }
    }

    /// Returns modules that directly or indirectly consume `module`, excluding
    /// `module`. Each consumer appears once, and cycles end the traversal.
    pub(super) fn transitive(&self, module: &ModuleId) -> Vec<ModuleId> {
        let mut pending = vec![module.clone()];
        let mut seen = BTreeSet::from([module.clone()]);
        let mut dependents = Vec::new();
        while let Some(base) = pending.pop() {
            for consumer in self.consumers.get(&base).into_iter().flatten() {
                if seen.insert(consumer.clone()) {
                    dependents.push(consumer.clone());
                    pending.push(consumer.clone());
                }
            }
        }
        dependents
    }
}
