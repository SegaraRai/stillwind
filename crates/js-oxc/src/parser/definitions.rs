use super::{
    CallExpression, CompileError, Expression, GetSpan, HandleDefinition, MacroExtractor,
    PropertyKey, ScopeDefinition, Span, Spanned, StyleInput, is_marker_label,
};

impl<'a> MacroExtractor<'a> {
    pub(super) fn collect_scope_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ScopeDefinition> {
        let label = match call.arguments.as_slice() {
            [] => None,
            [argument] => {
                if let Expression::StringLiteral(label) = argument.to_expression() {
                    Some(self.string_literal(label))
                } else {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE001",
                        "style.scope() accepts an optional literal name",
                        self.source_span(argument.span()),
                    ));
                    return None;
                }
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    "SHSCOPE001",
                    "style.scope() accepts at most one literal name",
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        if label.as_ref().is_some_and(|label| !is_marker_label(label)) {
            self.diagnostics.push(CompileError::new(
                "SHSCOPE001",
                "style.scope() names may contain only ASCII letters, digits, `_`, and `-`",
                label.expect("checked above").span,
            ));
            return None;
        }
        Some(ScopeDefinition {
            module_path: stillwind_source::normalized_module_path(self.source.path()),
            name,
            label,
            call_span: self.source_span(call.span),
        })
    }

    pub(super) fn collect_ref_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<HandleDefinition> {
        let (scope, label) = match call.arguments.as_slice() {
            [] => (None, None),
            [argument] => match argument.to_expression() {
                Expression::StringLiteral(label) => (None, Some(self.string_literal(label))),
                expression => {
                    if let Some(scope) = self.scope_reference(expression) {
                        (Some(scope), None)
                    } else {
                        self.diagnostics.push(CompileError::new(
                            "SHREF001",
                            "style.ref() accepts an optional literal name or a scope identity",
                            self.source_span(argument.span()),
                        ));
                        return None;
                    }
                }
            },
            [scope, label] => {
                let Some(scope) = self.scope_reference(scope.to_expression()) else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF001",
                        "the first style.ref() argument must be an identity returned by style.scope()",
                        self.source_span(scope.span()),
                    ));
                    return None;
                };
                let Expression::StringLiteral(label) = label.to_expression() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF001",
                        "the second style.ref() argument must be a literal name",
                        self.source_span(label.span()),
                    ));
                    return None;
                };
                (Some(scope), Some(self.string_literal(label)))
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    "SHREF001",
                    "style.ref() accepts (), (name), (scope), or (scope, name)",
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        if label.as_ref().is_some_and(|label| !is_marker_label(label)) {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "style.ref() names may contain only ASCII letters, digits, `_`, and `-`",
                label.expect("checked above").span,
            ));
            return None;
        }
        Some(HandleDefinition {
            module_path: stillwind_source::normalized_module_path(self.source.path()),
            name,
            label,
            scope,
            call_span: self.source_span(call.span),
        })
    }

    pub(super) fn handle_tagged_selector(
        &mut self,
        key: &PropertyKey<'a>,
    ) -> Option<(Vec<HandleDefinition>, Spanned<String>)> {
        let PropertyKey::TaggedTemplateExpression(tagged) = key else {
            return None;
        };
        let Some(handles) = self.handle_references(&tagged.tag) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional ref selectors require TypeScript checker facts",
                self.source_span(tagged.tag.span()),
            ));
            return None;
        };
        if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must be a template literal without substitutions",
                self.source_span(tagged.quasi.span),
            ));
            return None;
        }
        let Some(cooked) = tagged.quasi.quasis[0].value.cooked.as_ref() else {
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must be a valid static template literal",
                self.source_span(tagged.quasi.span),
            ));
            return None;
        };
        let template_span = self.source_span(tagged.quasi.span);
        let selector = Spanned::new(
            cooked.to_string(),
            Span::new(
                template_span.file,
                template_span.start.0 + 1,
                template_span.end.0.saturating_sub(1),
            ),
        );
        if !selector.value.starts_with('&') {
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must start with `&`",
                selector.span,
            ));
            return None;
        }
        for handle in &handles {
            self.record_handle_use(handle, "selector", self.source_span(tagged.span));
        }
        Some((handles, selector))
    }

    pub(super) fn collect_dynamic_handle_attachment(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) -> bool {
        if let Some(scopes) = self.scope_references(expression) {
            for scope in &scopes {
                self.record_scope_use(scope);
            }
            if scopes.len() == 1 {
                inputs.push(StyleInput::ScopeAttachment(scopes[0].clone()));
            } else {
                inputs.push(StyleInput::DynamicScopeAttachment {
                    value: self.source_span(expression.span()),
                    scopes,
                });
            }
            return true;
        }
        if let Some(handles) = self.handle_references(expression) {
            for handle in &handles {
                self.record_handle_use(handle, "ref", self.source_span(expression.span()));
            }
            if handles.len() == 1 {
                inputs.push(StyleInput::HandleAttachment(handles[0].clone()));
            } else {
                inputs.push(StyleInput::DynamicHandleAttachment {
                    value: self.source_span(expression.span()),
                    handles,
                });
            }
            return true;
        }
        false
    }
}
