//! Pairs host checker facts with the source scans supplied in the same order.

use std::collections::BTreeMap;

use stillwind_ir::{ModuleScan, TypeScriptFactsProvider};
use stillwind_semantic::{
    AssetMetadata, CheckedValue, ExportedValue, IdentityId, TypeFact, TypeFacts, TypeFactsUpdate,
};
use stillwind_source::Fingerprint;

use crate::source_input::HostPreparedSource;

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
                let assets = mentioned_assets(source.type_facts(), &assets);
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
                        assets,
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

/// Returns the entries of `assets` for the finite candidates of `facts`.
///
/// Fact-based extraction reads asset metadata only for these candidates, so a
/// module's semantic revision does not depend on assets its facts do not mention.
fn mentioned_assets(
    facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> BTreeMap<IdentityId, AssetMetadata> {
    facts
        .iter()
        .filter_map(|fact| match &fact.value {
            CheckedValue::Asset(
                ExportedValue::Styles(value)
                | ExportedValue::Refs(value)
                | ExportedValue::Keyframes(value)
                | ExportedValue::ViewTransitions(value),
            ) => value.finite_candidates(),
            CheckedValue::NonAsset => None,
        })
        .flatten()
        .filter_map(|identity| {
            assets
                .get_key_value(identity)
                .map(|(identity, metadata)| (identity.clone(), metadata.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use stillwind_semantic::{AssetKind, SymbolicValue};
    use stillwind_source::ModuleId;

    use super::*;

    #[test]
    fn modules_receive_only_the_asset_metadata_their_facts_mention() {
        let identity = |start| {
            IdentityId::new(
                AssetKind::Style,
                ModuleId::from("base.tsx"),
                start,
                start + 1,
                "base",
            )
        };
        let metadata = |label: &str| AssetMetadata {
            label: Some(label.into()),
        };
        let assets = BTreeMap::from([
            (identity(0), metadata("mentioned")),
            (identity(2), metadata("other")),
        ]);
        let facts = [
            TypeFact {
                start: 0,
                end: 1,
                value: ExportedValue::Styles(
                    SymbolicValue::finite(AssetKind::Style, [identity(0), identity(4)]).unwrap(),
                )
                .into(),
            },
            TypeFact {
                start: 1,
                end: 2,
                value: CheckedValue::NonAsset,
            },
        ];
        assert_eq!(
            mentioned_assets(&facts, &assets),
            BTreeMap::from([(identity(0), metadata("mentioned"))])
        );
        assert!(mentioned_assets(&[], &assets).is_empty());
    }
}
