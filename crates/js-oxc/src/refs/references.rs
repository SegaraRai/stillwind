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

use stillwind_ir::{BoundaryRef, RefDefinition};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, RefAlternative, SymbolicValue};
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
                if !refs.contains(&definition) {
                    refs.push(definition);
                }
            }
            return Some(refs);
        }
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Refs(alternatives)) = self.type_fact(span) {
            return self.refs_from_alternatives(alternatives);
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(definition) = self.ref_symbols.get(&symbol_id) {
            return Some(vec![definition.clone()]);
        }
        let ExportedValue::Refs(alternatives) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        self.refs_from_alternatives(alternatives)
    }

    /// Expands correlated alternatives into the ref definitions they denote.
    ///
    /// Each alternative keeps its own permitted boundary: an alternative with a finite
    /// boundary set expands to one definition per boundary, while an alternative whose
    /// boundary set is a universe yields `None`, because no single boundary could be
    /// chosen without changing the selector's meaning.
    pub(crate) fn refs_from_alternatives(
        &self,
        alternatives: &[RefAlternative],
    ) -> Option<Vec<RefDefinition>> {
        let mut refs = Vec::new();
        for alternative in alternatives {
            match &alternative.permitted_boundaries {
                None => refs.push(self.imported_ref(alternative)?),
                Some(boundaries) => {
                    for boundary in boundaries.finite_candidates()? {
                        let mut alternative = alternative.clone();
                        alternative.permitted_boundaries =
                            Some(SymbolicValue::exact(boundary.clone()));
                        refs.push(self.imported_ref(&alternative)?);
                    }
                }
            }
        }
        Some(refs)
    }

    /// Resolves an expression to exactly one boundary ref, or `None`.
    ///
    /// A boundary has to be statically identified: each candidate of a plural value
    /// would need its own boundary in the selector, so several candidates are reported
    /// by the caller instead of being narrowed to one.
    pub(crate) fn boundary_reference(&self, expression: &Expression<'a>) -> Option<BoundaryRef> {
        let refs = self.ref_references(expression)?;
        let [definition] = refs.as_slice() else {
            return None;
        };
        Some(Self::boundary_of(definition))
    }

    /// The boundary view of one resolved ref declaration.
    ///
    /// A boundary carries no selector responsibilities of its own, so the
    /// projection-free declaration fields are all a bounded ref keeps.
    fn boundary_of(definition: &RefDefinition) -> BoundaryRef {
        BoundaryRef {
            module_path: definition.module_path.clone(),
            binding: definition.binding.clone(),
            label: definition.label.clone(),
            call_span: definition.call_span,
        }
    }

    /// Rebuilds a boundary from a ref identity declared elsewhere.
    ///
    /// A locally declared ref is reused when the identity matches, so local and
    /// imported boundaries compare equal; otherwise the boundary is synthesized with
    /// the identity's declaration span and the host's metadata label.
    pub(crate) fn imported_boundary(&self, identity: &IdentityId) -> BoundaryRef {
        if let Some(definition) = self
            .ref_definitions
            .iter()
            .find(|definition| self.identity(AssetKind::Ref, &definition.binding) == *identity)
        {
            return Self::boundary_of(definition);
        }
        let span = Span::new(self.original.id(), identity.start, identity.end);
        let metadata = self.imported_assets.get(identity);
        BoundaryRef {
            module_path: identity.module_path.clone(),
            binding: Spanned::new(identity.binding_name.clone(), span),
            label: metadata
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span)),
            call_span: span,
        }
    }

    /// Rebuilds a ref definition from a correlated alternative.
    ///
    /// Returns `None` when the alternative's permitted boundaries are not exactly one
    /// finite candidate, because a ref definition carries at most one boundary.
    pub(crate) fn imported_ref(&self, alternative: &RefAlternative) -> Option<RefDefinition> {
        let span = Span::new(
            self.original.id(),
            alternative.reference.start,
            alternative.reference.end,
        );
        let metadata = self.imported_assets.get(&alternative.reference);
        let boundary = match &alternative.permitted_boundaries {
            None => None,
            Some(boundaries) => {
                let candidates = boundaries.finite_candidates()?;
                if candidates.len() != 1 {
                    return None;
                }
                Some(self.imported_boundary(candidates.first()?))
            }
        };
        let mut definition = self
            .ref_definitions
            .iter()
            .find(|definition| {
                self.identity(AssetKind::Ref, &definition.binding) == alternative.reference
            })
            .cloned()
            .unwrap_or_else(|| RefDefinition {
                module_path: alternative.reference.module_path.clone(),
                binding: Spanned::new(alternative.reference.binding_name.clone(), span),
                label: metadata
                    .and_then(|metadata| metadata.label.clone())
                    .map(|label| Spanned::new(label, span)),
                boundary: None,
                call_span: span,
            });
        definition.boundary = boundary;
        Some(definition)
    }
}
