//! Pairs host checker facts with the source scans supplied in the same order.

use std::collections::BTreeMap;

use stillwind_ir::{ModuleScan, TypeScriptFactsProvider};
use stillwind_semantic::{
    AssetMetadata, CheckedValue, ExportedValue, IdentityId, StoredVariantRecord, TypeFact,
    TypeFacts, TypeFactsUpdate,
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

/// Returns the entries of `assets` for the finite candidates of `facts`, and for the
/// refs that the records of mentioned stored variants name.
///
/// Fact-based extraction reads asset metadata only for these identities, so a
/// module's semantic revision does not depend on assets its facts do not mention.
/// Expanding an imported stored variant reads the labels of its refs, which the
/// facts of the using module do not name.
fn mentioned_assets(
    facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> BTreeMap<IdentityId, AssetMetadata> {
    let entry = |identity: &IdentityId| {
        assets
            .get_key_value(identity)
            .map(|(identity, metadata)| (identity.clone(), metadata.clone()))
    };
    let mentioned: BTreeMap<_, _> = facts
        .iter()
        .filter_map(|fact| match &fact.value {
            CheckedValue::Asset(
                ExportedValue::Styles(value)
                | ExportedValue::Refs(value)
                | ExportedValue::Keyframes(value)
                | ExportedValue::ViewTransitions(value)
                | ExportedValue::StoredVariants(value),
            ) => value.finite_candidates(),
            CheckedValue::NonAsset => None,
        })
        .flatten()
        .filter_map(entry)
        .collect();
    let refs: Vec<_> = mentioned
        .values()
        .filter_map(|metadata| metadata.stored_variant.as_ref())
        .flat_map(StoredVariantRecord::refs)
        .filter_map(entry)
        .collect();
    mentioned.into_iter().chain(refs).collect()
}

#[cfg(test)]
mod tests {
    use stillwind_semantic::{AssetKind, DeclarationKey, StoredVariantSegment, SymbolicValue};
    use stillwind_source::ModuleId;

    use super::*;

    #[test]
    fn modules_receive_only_the_asset_metadata_their_facts_mention() {
        let identity = |start| {
            IdentityId::new(
                AssetKind::Style,
                ModuleId::from("base.tsx"),
                DeclarationKey::top(start),
                "base",
            )
        };
        let metadata = |label: &str| AssetMetadata {
            label: Some(label.into()),
            stored_variant: None,
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

    #[test]
    fn mentioned_stored_variants_bring_the_metadata_of_their_refs() {
        let module = || ModuleId::from("base.tsx");
        let stored = IdentityId::new(
            AssetKind::StoredVariant,
            module(),
            DeclarationKey::top(0),
            "stored",
        );
        let named = IdentityId::new(AssetKind::Ref, module(), DeclarationKey::top(2), "named");
        let other = IdentityId::new(AssetKind::Ref, module(), DeclarationKey::top(4), "other");
        let stored_metadata = AssetMetadata {
            label: None,
            stored_variant: Some(StoredVariantRecord {
                segments: vec![StoredVariantSegment::Container {
                    refs: vec![named.clone()],
                    query: "@sm".into(),
                }],
            }),
        };
        let label = |label: &str| AssetMetadata {
            label: Some(label.into()),
            stored_variant: None,
        };
        let assets = BTreeMap::from([
            (stored.clone(), stored_metadata.clone()),
            (named.clone(), label("named")),
            (other, label("other")),
        ]);
        let facts = [TypeFact {
            start: 0,
            end: 1,
            value: ExportedValue::StoredVariants(SymbolicValue::exact(stored.clone())).into(),
        }];
        assert_eq!(
            mentioned_assets(&facts, &assets),
            BTreeMap::from([(stored, stored_metadata), (named, label("named"))])
        );
    }
}
