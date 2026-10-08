//! Pairs host checker facts with the source scans supplied in the same order.

use crate::source_input::HostPreparedSource;
use std::collections::BTreeMap;
use stillwind_ir::{ModuleScan, TypeScriptFactsProvider};
use stillwind_semantic::{TypeFacts, TypeFactsUpdate};
use stillwind_source::Fingerprint;

/// Adapter for facts already produced by the host's long-lived TS7 service.
///
/// The host pairs its facts with the scans it sent; this adapter only wraps them with
/// the module and asset metadata the domain type requires.
pub(super) struct ProvidedTypeScriptFacts<'a> {
    /// Host sources in the same order as the scans passed to `update`.
    sources: &'a [HostPreparedSource],
}

impl<'a> ProvidedTypeScriptFacts<'a> {
    pub(super) fn new(sources: &'a [HostPreparedSource]) -> Self {
        Self { sources }
    }
}

impl TypeScriptFactsProvider for ProvidedTypeScriptFacts<'_> {
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate {
        let assets: BTreeMap<_, _> = scans
            .iter()
            .flat_map(|scan| scan.summary.assets.iter())
            .map(|asset| (asset.identity.clone(), asset.metadata.clone()))
            .collect();
        let modules = scans
            .iter()
            .zip(self.sources)
            .map(|(scan, source)| {
                let semantic_revision =
                    Fingerprint::of(format!("{:?}{assets:?}", source.type_facts()));
                (
                    scan.module_id.clone(),
                    TypeFacts {
                        schema: TypeFacts::SCHEMA,
                        module_id: scan.module_id.clone(),
                        source_revision: scan.source_revision.clone(),
                        semantic_revision,
                        facts: source.type_facts().to_vec(),
                        assets: assets.clone(),
                    },
                )
            })
            .collect();
        TypeFactsUpdate {
            modules,
            diagnostics: Vec::new(),
        }
    }
}
