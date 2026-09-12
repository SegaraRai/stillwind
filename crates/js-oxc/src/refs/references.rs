//! Resolving an expression back to the refs and scopes it may denote.
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

use stillwind_ir::{HandleDefinition, ScopeDefinition};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, RefAlternative, SymbolicValue};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to exactly one ref, or `None`.
    ///
    /// Prefer this over [`Self::handle_references`] wherever a single identity is
    /// required; an ambiguous value must not be resolved by choosing.
    pub(crate) fn handle_reference(&self, expression: &Expression<'a>) -> Option<HandleDefinition> {
        let handles = self.handle_references(expression)?;
        (handles.len() == 1).then(|| handles[0].clone())
    }

    /// Resolves an expression to every ref it may denote.
    ///
    /// Records a type probe for non-literal expressions, so the host checker can answer
    /// on a later pass when local analysis cannot.
    pub(crate) fn handle_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<HandleDefinition>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Refs(alternatives)) =
            self.type_facts.get(&(span.start, span.end))
        {
            return self.handles_from_alternatives(alternatives);
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(handle) = self.handle_symbols.get(&symbol_id) {
            return Some(vec![handle.clone()]);
        }
        let ExportedValue::Refs(alternatives) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        self.handles_from_alternatives(alternatives)
    }

    /// Expands correlated alternatives into the handle definitions they denote.
    ///
    /// Each alternative keeps its own permitted scope: an alternative with a finite
    /// scope set expands to one handle per scope, while an alternative whose scope set
    /// is a universe yields `None`, because no single scope could be chosen without
    /// changing the selector's meaning.
    pub(crate) fn handles_from_alternatives(
        &self,
        alternatives: &[RefAlternative],
    ) -> Option<Vec<HandleDefinition>> {
        let mut handles = Vec::new();
        for alternative in alternatives {
            match &alternative.permitted_scopes {
                None => handles.push(self.imported_ref(alternative)?),
                Some(scopes) => {
                    for scope in scopes.finite_candidates()? {
                        let mut alternative = alternative.clone();
                        alternative.permitted_scopes = Some(SymbolicValue::exact(scope.clone()));
                        handles.push(self.imported_ref(&alternative)?);
                    }
                }
            }
        }
        Some(handles)
    }

    /// Resolves an expression to exactly one scope, or `None`.
    pub(crate) fn scope_reference(&self, expression: &Expression<'a>) -> Option<ScopeDefinition> {
        let scopes = self.scope_references(expression)?;
        (scopes.len() == 1).then(|| scopes[0].clone())
    }

    /// Resolves an expression to every scope it may denote.
    ///
    /// A scope value must be finite: a universe scope set means the checker could not
    /// narrow the value, so the use stays unresolved instead of being widened to every
    /// scope.
    pub(crate) fn scope_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<ScopeDefinition>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Scopes(value)) = self.type_facts.get(&(span.start, span.end)) {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.imported_scope(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(scope) = self.scope_symbols.get(&symbol_id) {
            return Some(vec![scope.clone()]);
        }
        let ExportedValue::Scopes(value) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        Some(
            value
                .finite_candidates()?
                .iter()
                .map(|identity| self.imported_scope(identity))
                .collect(),
        )
    }

    /// Rebuilds a scope definition from an identity declared elsewhere.
    ///
    /// A locally declared scope is reused when the identity matches, so local and
    /// imported scopes compare equal; otherwise the definition is synthesized with the
    /// identity's declaration span and the host's metadata label.
    pub(crate) fn imported_scope(&self, identity: &IdentityId) -> ScopeDefinition {
        if let Some(scope) = self
            .scope_definitions
            .iter()
            .find(|scope| self.identity(AssetKind::Scope, &scope.name) == *identity)
        {
            return scope.clone();
        }
        let span = Span::new(self.source.id(), identity.start, identity.end);
        let metadata = self.imported_assets.get(identity);
        ScopeDefinition {
            module_path: identity.module_path.clone(),
            name: Spanned::new(identity.binding_name.clone(), span),
            label: metadata
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span)),
            call_span: span,
        }
    }

    /// Rebuilds a ref definition from a correlated alternative.
    ///
    /// Returns `None` when the alternative's permitted scopes are not exactly one
    /// finite candidate, because a handle definition carries at most one scope.
    pub(crate) fn imported_ref(&self, alternative: &RefAlternative) -> Option<HandleDefinition> {
        let span = Span::new(
            self.source.id(),
            alternative.reference.start,
            alternative.reference.end,
        );
        let metadata = self.imported_assets.get(&alternative.reference);
        let scope = match &alternative.permitted_scopes {
            None => None,
            Some(scopes) => {
                let candidates = scopes.finite_candidates()?;
                if candidates.len() != 1 {
                    return None;
                }
                Some(self.imported_scope(candidates.first()?))
            }
        };
        let mut definition = self
            .handle_definitions
            .iter()
            .find(|handle| self.identity(AssetKind::Ref, &handle.name) == alternative.reference)
            .cloned()
            .unwrap_or_else(|| HandleDefinition {
                module_path: alternative.reference.module_path.clone(),
                name: Spanned::new(alternative.reference.binding_name.clone(), span),
                label: metadata
                    .and_then(|metadata| metadata.label.clone())
                    .map(|label| Spanned::new(label, span)),
                scope: None,
                call_span: span,
            });
        definition.scope = scope;
        Some(definition)
    }
}
