//! Extraction of `style.css()` calls.
//!
//! A call records its static utility literals with their group keys, and its options,
//! for the compiler, which replaces the call with a stylesheet string. The inputs are
//! compiled into CSS text, so every input must be static. An unsupported input or
//! option is left out of the IR with a lint finding: an input contributes no rules,
//! and an option lowers as if it were omitted.

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    Argument, ArrayExpressionElement, CallExpression, Expression, ObjectExpression,
    ObjectPropertyKind, PropertyKind,
};
use oxc::span::GetSpan;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{CssCall, CssInput};
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

/// Option names that `style.css()` accepts in its last argument.
const OPTIONS: [&str; 2] = ["template", "selector"];

impl<'a> MacroExtractor<'a> {
    /// Records one `style.css(...)` call.
    ///
    /// The last argument is the options object when it is an object literal whose
    /// properties all have one of the option names. Every other argument is an input.
    /// The call and its callee are marked as consumed, so the visitor does not report
    /// them again.
    ///
    /// The lowering of an enclosing style input omits the input that contains this
    /// call, so the visitor does not reach it. A call that an enclosing lowering kept
    /// anyway produces [`LintDiagnostic::NestedMacroCall`] and is not recorded.
    pub(crate) fn collect_css(&mut self, call: &CallExpression<'a>) {
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.span()));
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.get_inner_expression().span()));
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        let span = self.source_span(call.span);
        if self.inside_macro_call(span) {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::NestedMacroCall {
                    syntax: "style.css()".into(),
                },
                span,
            ));
            return;
        }
        let mut arguments = call.arguments.iter().collect::<Vec<_>>();
        let options = arguments
            .last()
            .and_then(|argument| argument.as_expression())
            .and_then(|expression| match expression.get_inner_expression() {
                Expression::ObjectExpression(object) if is_options(object) => Some(object),
                _ => None,
            });
        if options.is_some() {
            arguments.pop();
        }
        let mut inputs = Vec::new();
        for argument in arguments {
            match argument {
                Argument::SpreadElement(spread) => {
                    self.css_spread(&spread.argument, spread.span, &[], &mut inputs);
                }
                _ => {
                    if let Some(expression) = argument.as_expression() {
                        self.css_input(expression, &[], &mut inputs);
                    }
                }
            }
        }
        let mut css = CssCall {
            span,
            inputs,
            template: None,
            selector: None,
        };
        if let Some(options) = options {
            self.css_options(options, &mut css);
        }
        self.css_calls.push(css);
    }

    /// Appends the utility literals of one input with the enclosing group `variants`.
    fn css_input(
        &mut self,
        expression: &Expression<'a>,
        variants: &[Spanned<LeanStr>],
        inputs: &mut Vec<CssInput>,
    ) {
        match expression.get_inner_expression() {
            Expression::NullLiteral(_) => {}
            Expression::BooleanLiteral(value) if !value.value => {}
            Expression::Identifier(identifier) if identifier.name == "undefined" => {}
            Expression::StringLiteral(literal) => {
                inputs.push(CssInput {
                    variants: variants.to_vec(),
                    utilities: self.string_literal(literal),
                });
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let Some(value) = template
                    .quasis
                    .first()
                    .and_then(|quasi| quasi.value.cooked.as_ref())
                else {
                    self.css_input_error(expression, "the template contains an invalid escape");
                    return;
                };
                let template_span = self.source_span(template.span);
                inputs.push(CssInput {
                    variants: variants.to_vec(),
                    utilities: Spanned::new(
                        value.to_lean_str(),
                        Span::new(
                            template_span.file,
                            template_span.start.0 + 1,
                            template_span.end.0.saturating_sub(1),
                        ),
                    ),
                });
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            self.css_spread(&spread.argument, spread.span, variants, inputs);
                        }
                        ArrayExpressionElement::Elision(_) => {}
                        _ => {
                            if let Some(value) = element.as_expression() {
                                self.css_input(value, variants, inputs);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                self.css_group(object, variants, inputs);
            }
            _ => {
                self.css_input_error(expression, "the input is not static");
            }
        }
    }

    /// Appends the inputs of a literal array spread. Any other spread is an unsupported
    /// input.
    fn css_spread(
        &mut self,
        argument: &Expression<'a>,
        span: oxc::span::Span,
        variants: &[Spanned<LeanStr>],
        inputs: &mut Vec<CssInput>,
    ) {
        if matches!(
            argument.get_inner_expression(),
            Expression::ArrayExpression(_)
        ) {
            self.css_input(argument, variants, inputs);
        } else {
            self.omit_input(span);
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidCssInput {
                    reason: "spreads require a literal array".into(),
                },
                self.source_span(span),
            ));
        }
    }

    /// Appends the inputs of a variant group object.
    ///
    /// A key that ends in `:` adds its variants to the values beneath it, and the keys
    /// `""` and `":"` add none. Literal object spreads are expanded in place. Any other
    /// property, such as an attribute, a custom property, `class`, or a computed key,
    /// is unsupported and contributes nothing.
    fn css_group(
        &mut self,
        object: &ObjectExpression<'a>,
        variants: &[Spanned<LeanStr>],
        inputs: &mut Vec<CssInput>,
    ) {
        for property in &object.properties {
            let property = match property {
                ObjectPropertyKind::SpreadProperty(spread) => {
                    if let Expression::ObjectExpression(object) =
                        spread.argument.get_inner_expression()
                    {
                        self.css_group(object, variants, inputs);
                    } else {
                        self.omit_input(spread.span);
                        self.lint_findings.push(LintFinding::new(
                            LintDiagnostic::InvalidCssInput {
                                reason: "object spreads require a literal object".into(),
                            },
                            self.source_span(spread.span),
                        ));
                    }
                    continue;
                }
                ObjectPropertyKind::ObjectProperty(property) => property,
            };
            let name = (!property.method && property.kind == PropertyKind::Init)
                .then(|| self.static_property_name(&property.key))
                .flatten();
            let Some(name) = name.filter(|name| name.value.is_empty() || name.value.ends_with(':'))
            else {
                self.omit_input(property.span);
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidCssInput {
                        reason: "object keys must be variant prefixes ending in `:`".into(),
                    },
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if name.value.is_empty() || name.value == ":" {
                self.css_input(&property.value, variants, inputs);
            } else {
                let mut nested = variants.to_vec();
                nested.push(name);
                self.css_input(&property.value, &nested, inputs);
            }
        }
    }

    /// Records the static options of `object` in `css`. Each unsupported value is
    /// reported and lowers as if the option were omitted.
    fn css_options(&mut self, object: &ObjectExpression<'a>, css: &mut CssCall) {
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let Some(name) = self.static_property_name(&property.key) else {
                continue;
            };
            let value = match property.value.get_inner_expression() {
                Expression::StringLiteral(literal) => Some(self.string_literal(literal)),
                Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                    template
                        .quasis
                        .first()
                        .and_then(|quasi| quasi.value.cooked.as_ref())
                        .map(|value| {
                            let span = self.source_span(template.span);
                            Spanned::new(
                                value.to_lean_str(),
                                Span::new(
                                    span.file,
                                    span.start.0 + 1,
                                    span.end.0.saturating_sub(1),
                                ),
                            )
                        })
                }
                _ => None,
            };
            let Some(value) = value else {
                self.omit_input(property.span);
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidCssOption {
                        reason: format_args!("`{}` requires a string literal", name.value)
                            .to_lean_str(),
                    },
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            if name.value == "template" {
                css.template = Some(value);
            } else {
                css.selector = Some(value);
            }
        }
    }

    fn css_input_error(&mut self, expression: &Expression<'a>, reason: &str) {
        self.omit_input(expression.span());
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidCssInput {
                reason: reason.into(),
            },
            self.source_span(expression.span()),
        ));
    }
}

/// Whether `object` is a `style.css()` options object: it has at least one property,
/// and every property is a static key among [`OPTIONS`].
fn is_options(object: &ObjectExpression<'_>) -> bool {
    !object.properties.is_empty()
        && object.properties.iter().all(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            !property.computed
                && property
                    .key
                    .static_name()
                    .is_some_and(|name| OPTIONS.contains(&name.as_ref()))
        })
}
