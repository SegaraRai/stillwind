//! Resolving an expression back to the refs it may denote.
//!
//! Checker facts for the exact expression span take precedence over local symbolic
//! values. Imported assets are reconstructed from
//! their identities plus host metadata, so a ref declared in another module behaves
//! like a local one without this module seeing that file.
//!
//! A value that resolves to several candidates stays plural. Failure distinguishes
//! confirmed unsupported values from missing information about identities.

use oxc::ast::ast::Expression;

use stillwind_ir::RefDefinition;
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{Span, Spanned};

use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to every ref it may denote.
    ///
    /// Records a type probe for non-literal expressions, so the host checker can answer
    /// on a later pass when local analysis cannot.
    pub(crate) fn ref_references(
        &self,
        expression: &Expression<'a>,
    ) -> Result<Vec<RefDefinition>, ResolutionFailure> {
        self.asset_candidates(expression, AssetKind::Ref)
            .map(|identities| {
                identities
                    .iter()
                    .map(|identity| self.imported_ref(identity))
                    .collect()
            })
    }

    /// Resolves a ref identity to a local definition or reconstructs its metadata.
    ///
    /// `identity` must identify a ref declaration. Local declarations retain their
    /// spans and labels. For external declarations, the host label is retained and
    /// spans use the declaration offsets in this module's source coordinate space.
    pub(crate) fn imported_ref(&self, identity: &IdentityId) -> RefDefinition {
        if let Some(definition) = self
            .ref_definitions
            .iter()
            .find(|definition| self.ref_identity(definition) == *identity)
        {
            return definition.clone();
        }
        let span = Span::new(self.original.id(), identity.start(), identity.end());
        let metadata = self.imported_assets.get(identity);
        RefDefinition {
            module_id: identity.module_id().clone(),
            binding: Spanned::new(identity.binding_name().to_string(), span),
            label: metadata
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span)),
            call_span: span,
        }
    }

    /// Returns the ref's declaration identity in original UTF-8 coordinates.
    ///
    /// Definitions collected from the analysis buffer need one projection mapping.
    /// Definitions reconstructed from checker facts already carry original offsets,
    /// so their spans are retained. The declaring module comes from `definition`.
    pub(crate) fn ref_identity(&self, definition: &RefDefinition) -> IdentityId {
        let binding = &definition.binding;
        let span =
            if binding.span.file == self.source.id() && self.source.id() != self.original.id() {
                (self.coordinates)(binding.span).unwrap_or(self.original.full_span())
            } else {
                binding.span
            };
        IdentityId::new(
            AssetKind::Ref,
            definition.module_id.clone(),
            span.start.0,
            span.end.0,
            binding.value.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;

    use oxc::semantic::Scoping;

    use stillwind_semantic::{AssetMetadata, SymbolicValue};
    use stillwind_source::{FileId, ModuleId, SourceFile};

    use super::*;

    #[test]
    fn ref_identity_maps_analysis_spans_once_and_preserves_original_modules() {
        let source = SourceFile::new(FileId(3), None, None, "const item = style.ref();").unwrap();
        let original =
            SourceFile::new(FileId(1), None, None, "prefix const item = style.ref();").unwrap();
        let module_id = ModuleId::from("local.ts");
        let scoping = Scoping::default();
        let metadata = BTreeMap::new();
        let mappings = Cell::new(0);
        let coordinates = |span: Span| {
            assert_eq!(span.file, source.id());
            mappings.set(mappings.get() + 1);
            Some(Span::new(original.id(), span.start.0 + 7, span.end.0 + 7))
        };
        let mut extractor = MacroExtractor::new(
            &source,
            &module_id,
            &scoping,
            &[],
            &metadata,
            &original,
            &coordinates,
        );
        let identity = IdentityId::new(AssetKind::Ref, module_id.clone(), 13, 17, "item");
        let reconstructed = extractor.imported_ref(&identity);
        assert_eq!(extractor.ref_identity(&reconstructed), identity);
        assert_eq!(mappings.get(), 0);
        let local = RefDefinition {
            module_id: module_id.clone(),
            binding: Spanned::new("item".into(), Span::new(source.id(), 6, 10)),
            label: None,
            call_span: Span::new(source.id(), 13, 24),
        };
        assert_eq!(extractor.ref_identity(&local), identity);
        assert_eq!(mappings.get(), 1);
        extractor.ref_definitions.push(local.clone());
        assert_eq!(extractor.imported_ref(&identity), local);
        let external = RefDefinition {
            module_id: "external.ts".into(),
            ..reconstructed
        };
        let mappings_before_external = mappings.get();
        assert_eq!(
            extractor.ref_identity(&external),
            IdentityId::new(AssetKind::Ref, "external.ts".into(), 13, 17, "item"),
        );
        assert_eq!(mappings.get(), mappings_before_external);
    }

    #[test]
    fn finite_ref_candidates_reuse_local_definitions_and_import_labels() {
        let source = SourceFile::new(FileId(1), None, None, "const local = style.ref();").unwrap();
        let module_id = ModuleId::from("local.ts");
        let scoping = Scoping::default();
        let imported = IdentityId::new(AssetKind::Ref, "other.ts".into(), 2, 10, "external");
        let metadata = BTreeMap::from([(
            imported.clone(),
            AssetMetadata {
                label: Some("label".into()),
            },
        )]);
        let mut extractor = MacroExtractor::new(
            &source,
            &module_id,
            &scoping,
            &[],
            &metadata,
            &source,
            &Some,
        );
        let local = RefDefinition {
            module_id: module_id.clone(),
            binding: Spanned::new("local".into(), Span::new(source.id(), 6, 11)),
            label: None,
            call_span: Span::new(source.id(), 14, 25),
        };
        let local_id = extractor.identity(AssetKind::Ref, &local.binding);
        extractor.ref_definitions.push(local.clone());
        let value = SymbolicValue::finite(AssetKind::Ref, [imported.clone(), local_id]).unwrap();
        let refs = value
            .finite_candidates()
            .unwrap()
            .iter()
            .map(|identity| extractor.imported_ref(identity))
            .collect::<Vec<_>>();
        assert_eq!(refs[0], local);
        assert_eq!(refs[1].module_id, *imported.module_id());
        assert_eq!(refs[1].binding.value, *imported.binding_name());
        assert_eq!(refs[1].label.as_ref().unwrap().value, "label");
        assert_eq!(refs[1].call_span, Span::new(source.id(), 2, 10));
    }
}
