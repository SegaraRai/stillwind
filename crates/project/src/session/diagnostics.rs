//! Retains reports and rejected source attempts independently of published output.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_diagnostics::{CompileError, DiagnosticBatch};
use stillwind_source::{ModuleId, SourceFile, SourceRegistry, SourceRegistryError};

/// Original source and fatal findings from an attempt that produced no analysis.
struct RejectedSourceAttempt {
    source: Arc<SourceFile>,
    diagnostics: Vec<CompileError>,
}

/// Latest diagnostic batches and attempts that produced no analysis.
#[derive(Clone, Default)]
pub(super) struct DiagnosticState {
    reported: BTreeMap<ModuleId, DiagnosticBatch>,
    rejected: BTreeMap<ModuleId, Arc<RejectedSourceAttempt>>,
}

impl DiagnosticState {
    /// Returns the last reported batch for each retained module.
    pub(super) fn batches(&self) -> &BTreeMap<ModuleId, DiagnosticBatch> {
        &self.reported
    }

    /// Replaces a module's batch for its latest attempted revision.
    pub(super) fn publish(&mut self, module: ModuleId, batch: DiagnosticBatch) {
        self.reported.insert(module, batch);
    }

    /// Keeps the source and fatal findings from an attempt that produced no analysis.
    pub(super) fn reject(
        &mut self,
        module: ModuleId,
        source: Arc<SourceFile>,
        diagnostics: Vec<CompileError>,
    ) {
        self.rejected.insert(
            module,
            Arc::new(RejectedSourceAttempt {
                source,
                diagnostics,
            }),
        );
    }

    /// Whether a module's latest attempt was rejected before analysis.
    pub(super) fn is_rejected(&self, module: &ModuleId) -> bool {
        self.rejected.contains_key(module)
    }

    /// Clears a rejection when a later attempt has an analysis.
    pub(super) fn clear_rejection(&mut self, module: &ModuleId) {
        self.rejected.remove(module);
    }

    /// Removes reports and rejected attempts when a producer leaves the project.
    pub(super) fn remove(&mut self, module: &ModuleId) {
        self.reported.remove(module);
        self.rejected.remove(module);
    }

    /// Returns rejected modules and their fatal findings in module order.
    pub(super) fn rejections(&self) -> impl Iterator<Item = (&ModuleId, &[CompileError])> {
        self.rejected
            .iter()
            .map(|(id, attempt)| (id, attempt.diagnostics.as_slice()))
    }

    /// Retains source handles that unresolved rejected attempts still need.
    /// Returns an error if a retained handle conflicts with a registered revision.
    pub(super) fn retain_sources(
        &self,
        sources: &mut SourceRegistry,
    ) -> Result<(), SourceRegistryError> {
        for attempt in self.rejected.values() {
            sources.retain(attempt.source.clone())?;
        }
        Ok(())
    }
}
