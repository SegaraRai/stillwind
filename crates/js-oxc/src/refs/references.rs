//! Resolving an expression back to the refs it may denote.
//!
//! Resolution order is fixed: checker facts for the exact expression span, then a local
//! declaration, then a local symbolic value. Imported assets are reconstructed from
//! their identities plus host metadata, so a ref declared in another module behaves
//! like a local one without this module seeing that file.
//!
//! A value that resolves to several candidates stays plural. Callers that need exactly
//! one candidate use the singular helpers, which return `None` instead of picking one.

use oxc::ast::ast::Expression;
use oxc::span::GetSpan;

use stillwind_ir::RefDefinition;
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to every ref it may denote.
    ///
    /// Records a type probe for non-literal expressions, so the host checker can answer
    /// on a later pass when local analysis cannot.
    pub(crate) fn ref_references(&self, expression: &Expression<'a>) -> Option<Vec<RefDefinition>> {
        let expression = expression.get_inner_expression();
        if let Expression::ConditionalExpression(conditional) = expression {
            let consequent = self.ref_references(&conditional.consequent);
            let alternate = self.ref_references(&conditional.alternate);
            let mut refs = consequent?;
            for definition in alternate? {
                if !refs
                    .iter()
                    .any(|candidate| self.ref_identity(candidate) == self.ref_identity(&definition))
                {
                    refs.push(definition);
                }
            }
            return Some(refs);
        }
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Refs(value)) = self.type_fact(span) {
            return self.refs_from_value(value);
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(definition) = self.ref_symbols.get(&symbol_id) {
            return Some(vec![definition.clone()]);
        }
        let ExportedValue::Refs(value) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        self.refs_from_value(value)
    }

    /// Resolves finite ref candidates to their declaration definitions.
    ///
    /// Returns `None` for a universe value, a non-ref kind, or a candidate of another
    /// kind. An empty finite set returns an empty vector. Candidate order follows
    /// `IdentityId` ordering, and local declaration metadata is reused when available.
    pub(crate) fn refs_from_value(&self, value: &SymbolicValue) -> Option<Vec<RefDefinition>> {
        if value.kind != AssetKind::Ref || value.validate().is_err() {
            return None;
        }
        Some(
            value
                .finite_candidates()?
                .iter()
                .map(|identity| self.imported_ref(identity))
                .collect(),
        )
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
        let span = Span::new(self.original.id(), identity.start, identity.end);
        let metadata = self.imported_assets.get(identity);
        RefDefinition {
            module_id: identity.module_id.clone(),
            binding: Spanned::new(identity.binding_name.clone(), span),
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
    use std::collections::{BTreeMap, BTreeSet};

    use oxc::semantic::Scoping;

    use stillwind_semantic::{AssetMetadata, Candidates};
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
        let refs = extractor.refs_from_value(&value).unwrap();
        assert_eq!(refs[0], local);
        assert_eq!(refs[1].module_id, imported.module_id);
        assert_eq!(refs[1].binding.value, imported.binding_name);
        assert_eq!(refs[1].label.as_ref().unwrap().value, "label");
        assert_eq!(refs[1].call_span, Span::new(source.id(), 2, 10));
    }

    #[test]
    fn ref_resolution_distinguishes_empty_universe_and_invalid_candidates() {
        let source = SourceFile::new(FileId(1), None, None, "").unwrap();
        let module_id = ModuleId::from("local.ts");
        let scoping = Scoping::default();
        let metadata = BTreeMap::new();
        let extractor = MacroExtractor::new(
            &source,
            &module_id,
            &scoping,
            &[],
            &metadata,
            &source,
            &Some,
        );
        let empty = SymbolicValue::finite(AssetKind::Ref, []).unwrap();
        assert_eq!(extractor.refs_from_value(&empty), Some(Vec::new()));
        assert_eq!(
            extractor.refs_from_value(&SymbolicValue::universe(AssetKind::Ref)),
            None
        );
        let wrong = IdentityId::new(AssetKind::Keyframes, "other.ts".into(), 0, 1, "frames");
        assert_eq!(
            extractor.refs_from_value(&SymbolicValue::exact(wrong.clone())),
            None
        );
        let malformed = SymbolicValue {
            kind: AssetKind::Ref,
            candidates: Candidates::Finite(BTreeSet::from([wrong])),
        };
        assert_eq!(extractor.refs_from_value(&malformed), None);
    }
}
