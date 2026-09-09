use super::{
    CallExpression, CompileError, Expression, GetSpan, HandleDefinition, HandleProjection,
    HandleUse, MacroExtractor, ScopeDefinition, ScopeUse, Span, Spanned, StaticMemberExpression,
    StringLiteral, StyleInput,
};

impl<'a> MacroExtractor<'a> {
    pub(super) fn collect_handle_projection(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        if let Expression::CallExpression(call) = expression
            && let Expression::StaticMemberExpression(member) = &call.callee
            && member.property.name.as_str() == "at"
        {
            return self.collect_container_query_projection(call, member);
        }
        let (handle_expression, projection) = match expression {
            Expression::StaticMemberExpression(member) => {
                let projection = match member.property.name.as_str() {
                    "anchor" => HandleProjection::Anchor,
                    "position" => HandleProjection::Position,
                    "anchorScope" => HandleProjection::Scope,
                    _ => return None,
                };
                (&member.object, projection)
            }
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                if member.property.name.as_str() != "container" {
                    return None;
                }
                let Some(argument) = call.arguments.first() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
                        self.source_span(call.span),
                    ));
                    return None;
                };
                let Expression::StringLiteral(value) = argument.to_expression() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
                        self.source_span(argument.span()),
                    ));
                    return None;
                };
                (
                    &member.object,
                    HandleProjection::Container(self.string_literal(value)),
                )
            }
            _ => return None,
        };
        let Some(handles) = self.handle_references(handle_expression) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional ref projections require TypeScript checker facts",
                self.source_span(expression.span()),
            ));
            return None;
        };
        let projection_name = match &projection {
            HandleProjection::Anchor => "anchor",
            HandleProjection::Position => "position",
            HandleProjection::Scope => "scope",
            HandleProjection::Container(_) => "container",
        };
        for handle in &handles {
            self.record_handle_use(handle, projection_name, self.source_span(expression.span()));
        }
        if handles.len() == 1 {
            Some(StyleInput::HandleProjection {
                handle: handles[0].clone(),
                projection,
            })
        } else {
            Some(StyleInput::DynamicHandleProjection {
                value: self.source_span(expression.span()),
                handles,
                projection,
            })
        }
    }

    pub(super) fn collect_container_query_projection(
        &mut self,
        call: &CallExpression<'a>,
        member: &StaticMemberExpression<'a>,
    ) -> Option<StyleInput> {
        if call.arguments.len() != 2 {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(handle) = self.handle_reference(&member.object) else {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "ref projections must use an identity returned by style.ref()",
                self.source_span(call.span),
            ));
            return None;
        };
        let (Expression::StringLiteral(query), Expression::StringLiteral(value)) = (
            call.arguments[0].to_expression(),
            call.arguments[1].to_expression(),
        ) else {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        };
        self.record_handle_use(&handle, "container-query", self.source_span(call.span));
        Some(StyleInput::ContainerQuery {
            handle,
            query: self.string_literal(query),
            value: self.string_literal(value),
        })
    }

    pub(super) fn record_handle_use(
        &mut self,
        handle: &HandleDefinition,
        projection: &'static str,
        span: Span,
    ) {
        self.handle_uses.push(HandleUse {
            name: handle.name.value.clone(),
            projection,
            span,
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }

    pub(super) fn record_scope_use(&mut self, scope: &ScopeDefinition) {
        self.scope_uses.push(ScopeUse {
            name: scope.name.value.clone(),
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }

    pub(super) fn string_literal(&self, literal: &StringLiteral<'a>) -> Spanned<String> {
        let literal_span = self.source_span(literal.span);
        let content_span = Span::new(
            literal_span.file,
            literal_span.start.0 + 1,
            literal_span.end.0.saturating_sub(1),
        );
        Spanned::new(literal.value.to_string(), content_span)
    }
}
