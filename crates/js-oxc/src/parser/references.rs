use super::{
    AssetReference, CallExpression, ExportedValue, Expression, GetSpan, HandleDefinition,
    IdentityId, ImportDeclaration, MacroExtractor, RefAlternative, ScopeDefinition, Span, Spanned,
    SymbolicValue,
};

impl<'a> MacroExtractor<'a> {
    pub(super) fn source_span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    pub(super) fn is_style_import(declaration: &ImportDeclaration<'a>) -> bool {
        declaration.source.value.as_str() == "stillwind/react"
    }

    pub(super) fn is_style_member(&self, expression: &Expression<'a>, property: &str) -> bool {
        let Expression::StaticMemberExpression(member) = expression else {
            return false;
        };
        let Expression::Identifier(identifier) = &member.object else {
            return false;
        };
        let Some(reference_id) = identifier.reference_id.get() else {
            return false;
        };
        self.scoping
            .get_reference(reference_id)
            .symbol_id()
            .is_some_and(|symbol| self.style_symbols.contains(&symbol))
            && member.property.name.as_str() == property
    }

    pub(super) fn is_style_ref_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "ref")
    }

    pub(super) fn is_style_scope_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "scope")
    }

    pub(super) fn is_style_animation_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "animation")
    }

    pub(super) fn is_style_view_transition_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "viewTransition")
    }

    pub(super) fn handle_reference(&self, expression: &Expression<'a>) -> Option<HandleDefinition> {
        let handles = self.handle_references(expression)?;
        (handles.len() == 1).then(|| handles[0].clone())
    }

    pub(super) fn handle_references(
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

    pub(super) fn handles_from_alternatives(
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

    pub(super) fn scope_reference(&self, expression: &Expression<'a>) -> Option<ScopeDefinition> {
        let scopes = self.scope_references(expression)?;
        (scopes.len() == 1).then(|| scopes[0].clone())
    }

    pub(super) fn scope_references(
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

    pub(super) fn asset_reference(&self, identity: &IdentityId) -> AssetReference {
        AssetReference {
            module_path: identity.module_path.clone(),
            name: Spanned::new(
                identity.binding_name.clone(),
                Span::new(self.source.id(), identity.start, identity.end),
            ),
        }
    }

    pub(super) fn animation_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
        if let Some(ExportedValue::Animations(value)) = self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let animation = self.animation_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            module_path: self.source.path().to_owned(),
            name: animation.clone(),
        }])
    }

    pub(super) fn view_transition_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
        if let Some(ExportedValue::ViewTransitions(value)) =
            self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let transition = self.view_transition_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            module_path: self.source.path().to_owned(),
            name: transition.clone(),
        }])
    }
}
