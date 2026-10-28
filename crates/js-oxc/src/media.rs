//! Extraction of `style.media()` calls.
//!
//! A call records its static variant arguments for the compiler, which replaces the
//! call with a media query list string. Unsupported arguments are left out of the IR
//! with a lint finding, because they match no environment.

use lean_string::ToLeanStr;
use oxc::ast::ast::{Argument, CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::MediaCall;
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Records one `style.media(...)` call.
    ///
    /// The call and its callee are marked as consumed, so the visitor does not report
    /// them again. Spread arguments, non-literal arguments, and a call without
    /// arguments produce [`LintDiagnostic::InvalidMediaArgument`].
    ///
    /// The lowering of an enclosing style input omits the input that contains this
    /// call, so the visitor does not reach it. A call that an enclosing lowering kept
    /// anyway produces [`LintDiagnostic::NestedMacroCall`] and is not recorded, because
    /// the enclosing replacement copies its source.
    pub(crate) fn collect_media(&mut self, call: &CallExpression<'a>) {
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.span()));
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.get_inner_expression().span()));
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        let span = self.source_span(call.span);
        if self.inside_macro_call(span) {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::NestedMacroCall {
                    syntax: "style.media()".into(),
                },
                span,
            ));
            return;
        }
        if call.arguments.is_empty() {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidMediaArgument {
                    reason: "the call has no arguments".into(),
                },
                span,
            ));
        }
        let mut arguments = Vec::new();
        for argument in &call.arguments {
            let reason = match argument {
                Argument::SpreadElement(_) => "spread arguments are unsupported",
                _ => match argument
                    .as_expression()
                    .map(Expression::get_inner_expression)
                {
                    Some(Expression::StringLiteral(literal)) => {
                        arguments.push(self.string_literal(literal));
                        continue;
                    }
                    Some(Expression::TemplateLiteral(template))
                        if template.expressions.is_empty() =>
                    {
                        if let Some(value) = template
                            .quasis
                            .first()
                            .and_then(|quasi| quasi.value.cooked)
                            .and_then(|value| self.unicode_string(value, template.span))
                        {
                            let template_span = self.source_span(template.span);
                            arguments.push(Spanned::new(
                                value.to_lean_str(),
                                Span::new(
                                    template_span.file,
                                    template_span.start.0 + 1,
                                    template_span.end.0.saturating_sub(1),
                                ),
                            ));
                            continue;
                        }
                        "the template contains an invalid escape"
                    }
                    _ => "the argument is not a string literal",
                },
            };
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidMediaArgument {
                    reason: reason.into(),
                },
                self.source_span(argument.span()),
            ));
        }
        self.media_calls.push(MediaCall { span, arguments });
    }

    /// Whether `span` lies inside a style call, CVA factory, `style.css()` call, or
    /// declaration call that this extraction has already recorded.
    pub(crate) fn inside_macro_call(&self, span: Span) -> bool {
        let contains = |outer: Span| outer.start <= span.start && span.end <= outer.end;
        self.style_calls.iter().any(|call| contains(call.span))
            || self.css_calls.iter().any(|call| contains(call.span))
            || self.cva_calls.iter().any(|call| contains(call.span))
            || self
                .ref_definitions
                .iter()
                .any(|definition| contains(definition.call_span))
            || self
                .keyframes_declarations
                .iter()
                .any(|definition| contains(definition.call_span))
            || self
                .view_transition_declarations
                .iter()
                .any(|definition| contains(definition.call_span))
    }
}
