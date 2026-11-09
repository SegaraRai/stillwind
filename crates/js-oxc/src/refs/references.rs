//! Resolving an expression back to the refs it may denote.
//!
//! Checker facts for the exact expression span take precedence over local symbolic
//! values. Imported assets are reconstructed from
//! their identities plus host metadata, so a ref declared in another module behaves
//! like a local one without this module seeing that file.
//!
//! A value that resolves to several candidates stays plural. Failure distinguishes
//! confirmed unsupported values from missing information about identities.

use lean_string::ToLeanStr;
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
    /// spans and labels. For external declarations, the host label is retained, and
    /// the spans are empty at the start of this module, because the offsets of the
    /// declaring module are not known here.
    pub(crate) fn imported_ref(&self, identity: &IdentityId) -> RefDefinition {
        if let Some(definition) = self
            .ref_definitions
            .iter()
            .find(|definition| ref_identity(definition) == *identity)
        {
            return definition.clone();
        }
        let span = Span::new(self.original.id(), 0, 0);
        let metadata = self.imported_assets.get(identity);
        RefDefinition {
            module_id: identity.module_id().clone(),
            binding: Spanned::new(identity.binding_name().to_lean_str(), span),
            key: identity.key().clone(),
            label: metadata
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span)),
            call_span: span,
        }
    }
}

/// Returns the declaration identity of the ref that `definition` declares.
pub(crate) fn ref_identity(definition: &RefDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        definition.module_id.clone(),
        definition.key.clone(),
        definition.binding.value.clone(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use oxc::semantic::Scoping;

    use stillwind_semantic::{AssetMetadata, DeclarationKey, SymbolicValue};
    use stillwind_source::{FileId, ModuleId, SourceFile};

    use super::*;

    #[test]
    fn ref_identities_use_the_declaration_key_and_module() {
        let source = SourceFile::new(FileId(3), None, None, "const item = style.ref();").unwrap();
        let module_id = ModuleId::from("local.ts");
        let scoping = Scoping::default();
        let metadata = BTreeMap::new();
        let mut extractor = MacroExtractor::new(
            &source,
            &module_id,
            &scoping,
            &[],
            &metadata,
            &source,
            &Some,
        );
        let key = DeclarationKey::new(vec!["Menu".into()], 1);
        let identity = IdentityId::new(AssetKind::Ref, module_id.clone(), key.clone(), "item");
        let reconstructed = extractor.imported_ref(&identity);
        assert_eq!(ref_identity(&reconstructed), identity);
        let local = RefDefinition {
            module_id: module_id.clone(),
            binding: Spanned::new("item".into(), Span::new(source.id(), 6, 10)),
            key,
            label: None,
            call_span: Span::new(source.id(), 13, 24),
        };
        extractor.ref_definitions.push(local.clone());
        assert_eq!(extractor.imported_ref(&identity), local);
        let external = RefDefinition {
            module_id: "external.ts".into(),
            ..local
        };
        assert_eq!(
            ref_identity(&external),
            IdentityId::new(
                AssetKind::Ref,
                "external.ts".into(),
                DeclarationKey::new(vec!["Menu".into()], 1),
                "item"
            ),
        );
    }

    #[test]
    fn finite_ref_candidates_reuse_local_definitions_and_import_labels() {
        let source = SourceFile::new(FileId(1), None, None, "const local = style.ref();").unwrap();
        let module_id = ModuleId::from("local.ts");
        let scoping = Scoping::default();
        let imported = IdentityId::new(
            AssetKind::Ref,
            "other.ts".into(),
            DeclarationKey::top(2),
            "external",
        );
        let metadata = BTreeMap::from([(
            imported.clone(),
            AssetMetadata {
                label: Some("label".into()),
                stored_variant: None,
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
            key: DeclarationKey::top(0),
            label: None,
            call_span: Span::new(source.id(), 14, 25),
        };
        let local_id = extractor.identity(AssetKind::Ref, &local.binding, &local.key);
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
        assert_eq!(refs[1].call_span, Span::new(source.id(), 0, 0));
    }
}
