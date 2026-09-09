use oxc::ast::ast::Expression;
use oxc::span::GetSpan;

use stillwind_ir::{HandleDefinition, ScopeDefinition};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, RefAlternative, SymbolicValue};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn handle_reference(&self, expression: &Expression<'a>) -> Option<HandleDefinition> {
        let handles = self.handle_references(expression)?;
        (handles.len() == 1).then(|| handles[0].clone())
    }

    pub(crate) fn handle_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<HandleDefinition>> {
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
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

    pub(crate) fn scope_reference(&self, expression: &Expression<'a>) -> Option<ScopeDefinition> {
        let scopes = self.scope_references(expression)?;
        (scopes.len() == 1).then(|| scopes[0].clone())
    }

    pub(crate) fn scope_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<ScopeDefinition>> {
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
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
