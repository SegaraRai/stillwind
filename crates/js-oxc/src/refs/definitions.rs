use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_ir::{HandleDefinition, ScopeDefinition, StyleInput, StyleVariant};
use stillwind_source::{CompileError, Span, Spanned, normalized_module_path};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn collect_scope_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ScopeDefinition> {
        let label = match call.arguments.as_slice() {
            [] => None,
            [argument] => {
                if let Some(Expression::StringLiteral(label)) = argument.as_expression() {
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
        if let Some(label) = &label
            && !is_marker_label(label)
        {
            self.diagnostics.push(CompileError::new(
                "SHSCOPE001",
                "style.scope() names may contain only ASCII letters, digits, `_`, and `-`",
                label.span,
            ));
            return None;
        }
        Some(ScopeDefinition {
            module_path: normalized_module_path(self.source.path()),
            name,
            label,
            call_span: self.source_span(call.span),
        })
    }

    pub(crate) fn collect_ref_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<HandleDefinition> {
        let (scope, label) = match call.arguments.as_slice() {
            [] => (None, None),
            [argument] => match argument.as_expression() {
                Some(Expression::StringLiteral(label)) => (None, Some(self.string_literal(label))),
                expression => {
                    if let Some(scope) =
                        expression.and_then(|expression| self.scope_reference(expression))
                    {
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
                let Some(scope) = scope
                    .as_expression()
                    .and_then(|scope| self.scope_reference(scope))
                else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF001",
                        "the first style.ref() argument must be an identity returned by style.scope()",
                        self.source_span(scope.span()),
                    ));
                    return None;
                };
                let Some(Expression::StringLiteral(label)) = label.as_expression() else {
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
        if let Some(label) = &label
            && !is_marker_label(label)
        {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "style.ref() names may contain only ASCII letters, digits, `_`, and `-`",
                label.span,
            ));
            return None;
        }
        Some(HandleDefinition {
            module_path: normalized_module_path(self.source.path()),
            name,
            label,
            scope,
            call_span: self.source_span(call.span),
        })
    }

    pub(crate) fn ref_variant(&mut self, expression: &Expression<'a>) -> Option<StyleVariant> {
        let (reference, mut prefix) = match expression {
            Expression::TaggedTemplateExpression(tagged) => {
                if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
                    self.diagnostics.push(CompileError::new(
                        "SHREF005",
                        "ref variants require a static variant string",
                        self.source_span(tagged.quasi.span),
                    ));
                    return None;
                }
                let cooked = tagged.quasi.quasis[0].value.cooked.as_ref()?;
                let span = self.source_span(tagged.quasi.span);
                (
                    &tagged.tag,
                    Spanned::new(
                        cooked.to_string(),
                        Span::new(span.file, span.start.0 + 1, span.end.0.saturating_sub(1)),
                    ),
                )
            }
            Expression::CallExpression(call) if call.arguments.is_empty() && !call.optional => (
                &call.callee,
                Spanned::new(String::new(), self.source_span(call.span)),
            ),
            Expression::CallExpression(call) if call.arguments.len() == 1 && !call.optional => {
                let Expression::StringLiteral(value) = call.arguments[0].as_expression()? else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF005",
                        "ref variants require a static variant string",
                        self.source_span(call.span),
                    ));
                    return None;
                };
                (&call.callee, self.string_literal(value))
            }
            _ => return None,
        };
        let Some(handles) = self.handle_references(reference) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "ref variants require a statically identified style.ref()",
                self.source_span(reference.span()),
            ));
            return None;
        };
        if !prefix.value.is_empty() && !prefix.value.ends_with(':') {
            prefix.value.push(':');
        }
        for handle in &handles {
            self.record_handle_use(handle, "selector", self.source_span(expression.span()));
        }
        Some(StyleVariant::Ref { handles, prefix })
    }

    pub(crate) fn collect_dynamic_handle_attachment(
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

fn is_marker_label(label: &Spanned<String>) -> bool {
    !label.value.is_empty()
        && label
            .value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn labels_allow_only_nonempty_ascii_marker_characters() {
        for (label, expected) in [
            ("a-b_2", true),
            ("", false),
            ("a b", false),
            ("日本語", false),
            ("a:b", false),
        ] {
            assert_eq!(
                is_marker_label(&Spanned::new(label.to_owned(), Span::new(FileId(1), 0, 0))),
                expected
            );
        }
    }
}
