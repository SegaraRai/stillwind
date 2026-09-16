//! Validation and extraction of `style.ref()` declarations and ref variants.
//!
//! A ref accepts an optional literal label, and may instead be bounded by another
//! ref. Labels retain their decoded text for marker generation; the compiler handles
//! whitespace normalization and CSS/JavaScript serialization.
//!
//! Ref variants accept the tagged-template, no-argument call and single-literal call
//! forms; every other shape is reported. The prefix is normalized to end with `:` so
//! later stages can concatenate without re-checking.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{RefDefinition, StyleInput, StyleVariant};
use stillwind_source::{Span, Spanned, normalized_module_path};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Validates one `style.ref()` declaration.
    ///
    /// Accepts `()`, `(label)`, `(boundary)` and `(boundary, label)`. The boundary
    /// argument must resolve to exactly one ref identity, so a ref can never be bounded
    /// by an arbitrary value.
    pub(crate) fn collect_ref_definition(
        &mut self,
        binding: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<RefDefinition> {
        let (boundary, label) = match call.arguments.as_slice() {
            [] => (None, None),
            [argument] => match argument.as_expression() {
                Some(Expression::StringLiteral(label)) => (None, Some(self.string_literal(label))),
                expression => {
                    if let Some(boundary) =
                        expression.and_then(|expression| self.boundary_reference(expression))
                    {
                        (Some(boundary), None)
                    } else {
                        self.diagnostics.push(CompileError::new(
                            CompileErrorKind::InvalidRefArgument,
                            self.source_span(argument.span()),
                        ));
                        return None;
                    }
                }
            },
            [boundary, label] => {
                let Some(boundary) = boundary
                    .as_expression()
                    .and_then(|boundary| self.boundary_reference(boundary))
                else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidRefBoundary,
                        self.source_span(boundary.span()),
                    ));
                    return None;
                };
                let Some(Expression::StringLiteral(label)) = label.as_expression() else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidRefLabelArgument,
                        self.source_span(label.span()),
                    ));
                    return None;
                };
                (Some(boundary), Some(self.string_literal(label)))
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidRefArguments,
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        Some(RefDefinition {
            module_path: normalized_module_path(self.source.path()),
            binding,
            label,
            boundary,
            call_span: self.source_span(call.span),
        })
    }

    /// Extracts one ref variant from a computed key or template substitution.
    ///
    /// The tag or callee must resolve to a ref, and a tag must be static (no
    /// interpolations). `None` means the expression is not a ref variant, which lets
    /// callers try other forms before reporting an error.
    pub(crate) fn ref_variant(&mut self, expression: &Expression<'a>) -> Option<StyleVariant> {
        let (reference, mut prefix) = match expression {
            Expression::TaggedTemplateExpression(tagged) => {
                if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::NonStaticRefVariant,
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
                        CompileErrorKind::NonStaticRefVariant,
                        self.source_span(call.span),
                    ));
                    return None;
                };
                (&call.callee, self.string_literal(value))
            }
            _ => return None,
        };
        if let Expression::StaticMemberExpression(member) = reference
            && member.property.name == "at"
        {
            let Some(refs) = self.ref_references(&member.object) else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::UnidentifiedRefVariant,
                    self.source_span(member.object.span()),
                ));
                return None;
            };
            if prefix.value.is_empty() {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidContainerQuery,
                    prefix.span,
                ));
                return None;
            }
            for definition in &refs {
                self.record_ref_use(
                    definition,
                    "container-query",
                    self.source_span(expression.span()),
                );
            }
            return Some(StyleVariant::Container {
                refs,
                query: prefix,
            });
        }
        let Some(refs) = self.ref_references(reference) else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::UnidentifiedRefVariant,
                self.source_span(reference.span()),
            ));
            return None;
        };
        if !prefix.value.is_empty() && !prefix.value.ends_with(':') {
            prefix.value.push(':');
        }
        for definition in &refs {
            self.record_ref_use(definition, "selector", self.source_span(expression.span()));
        }
        Some(StyleVariant::Ref { refs, prefix })
    }

    /// Recognizes a ref used as a `style()` argument.
    ///
    /// Returns true when the expression was consumed as an attachment; a set of several
    /// candidates becomes an explicitly dynamic attachment rather than one of them
    /// being chosen.
    pub(crate) fn collect_dynamic_ref_attachment(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) -> bool {
        if let Some(refs) = self.ref_references(expression) {
            for definition in &refs {
                self.record_ref_use(definition, "ref", self.source_span(expression.span()));
            }
            if refs.len() == 1 {
                inputs.push(StyleInput::RefAttachment(refs[0].clone()));
            } else {
                inputs.push(StyleInput::DynamicRefAttachment {
                    value: self.source_span(expression.span()),
                    refs,
                });
            }
            return true;
        }
        false
    }
}
