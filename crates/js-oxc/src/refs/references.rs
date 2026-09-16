//! Resolving an expression back to the refs and subtrees it may denote.
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

use stillwind_ir::{RefDefinition, SubtreeDefinition};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, RefAlternative, SymbolicValue};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to every ref it may denote.
    ///
    /// Records a type probe for non-literal expressions, so the host checker can answer
    /// on a later pass when local analysis cannot.
    pub(crate) fn ref_references(&self, expression: &Expression<'a>) -> Option<Vec<RefDefinition>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Refs(alternatives)) =
            self.type_facts.get(&(span.start, span.end))
        {
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
    /// Each alternative keeps its own permitted subtree: an alternative with a finite
    /// subtree set expands to one definition per subtree, while an alternative whose subtree
    /// set is a universe yields `None`, because no single subtree could be chosen without
    /// changing the selector's meaning.
    pub(crate) fn refs_from_alternatives(
        &self,
        alternatives: &[RefAlternative],
    ) -> Option<Vec<RefDefinition>> {
        let mut refs = Vec::new();
        for alternative in alternatives {
            match &alternative.permitted_subtrees {
                None => refs.push(self.imported_ref(alternative)?),
                Some(subtrees) => {
                    for subtree in subtrees.finite_candidates()? {
                        let mut alternative = alternative.clone();
                        alternative.permitted_subtrees =
                            Some(SymbolicValue::exact(subtree.clone()));
                        refs.push(self.imported_ref(&alternative)?);
                    }
                }
            }
        }
        Some(refs)
    }

    /// Resolves an expression to exactly one subtree, or `None`.
    pub(crate) fn subtree_reference(
        &self,
        expression: &Expression<'a>,
    ) -> Option<SubtreeDefinition> {
        let subtrees = self.subtree_references(expression)?;
        (subtrees.len() == 1).then(|| subtrees[0].clone())
    }

    /// Resolves an expression to every subtree it may denote.
    ///
    /// A subtree value must be finite: a universe subtree set means the checker could not
    /// narrow the value, so the use stays unresolved instead of being widened to every
    /// subtree.
    pub(crate) fn subtree_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<SubtreeDefinition>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Subtrees(value)) = self.type_facts.get(&(span.start, span.end)) {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.imported_subtree(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(subtree) = self.subtree_symbols.get(&symbol_id) {
            return Some(vec![subtree.clone()]);
        }
        let ExportedValue::Subtrees(value) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        Some(
            value
                .finite_candidates()?
                .iter()
                .map(|identity| self.imported_subtree(identity))
                .collect(),
        )
    }

    /// Rebuilds a subtree definition from an identity declared elsewhere.
    ///
    /// A locally declared subtree is reused when the identity matches, so local and
    /// imported subtrees compare equal; otherwise the definition is synthesized with the
    /// identity's declaration span and the host's metadata label.
    pub(crate) fn imported_subtree(&self, identity: &IdentityId) -> SubtreeDefinition {
        if let Some(subtree) = self
            .subtree_definitions
            .iter()
            .find(|subtree| self.identity(AssetKind::Subtree, &subtree.binding) == *identity)
        {
            return subtree.clone();
        }
        let span = Span::new(self.source.id(), identity.start, identity.end);
        let metadata = self.imported_assets.get(identity);
        SubtreeDefinition {
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
    /// Returns `None` when the alternative's permitted subtrees are not exactly one
    /// finite candidate, because a ref definition carries at most one subtree.
    pub(crate) fn imported_ref(&self, alternative: &RefAlternative) -> Option<RefDefinition> {
        let span = Span::new(
            self.source.id(),
            alternative.reference.start,
            alternative.reference.end,
        );
        let metadata = self.imported_assets.get(&alternative.reference);
        let subtree = match &alternative.permitted_subtrees {
            None => None,
            Some(subtrees) => {
                let candidates = subtrees.finite_candidates()?;
                if candidates.len() != 1 {
                    return None;
                }
                Some(self.imported_subtree(candidates.first()?))
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
                subtree: None,
                call_span: span,
            });
        definition.subtree = subtree;
        Some(definition)
    }
}
