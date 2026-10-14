//! Dispatches style arguments and records checker probes while preserving source order.

use oxc::ast::ast::{ArrayExpression, ArrayExpressionElement, CallExpression, Expression};
use oxc::span::GetSpan;
use oxc::syntax::operator::LogicalOperator;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{ClassExpression, ConditionalBranches, StyleInput};
use stillwind_semantic::{AssetKind, CheckedValue, ExportedValue, IdentityId};

use crate::local_span::LocalSpan;
use crate::resolution::{ResolutionFailure, cannot_carry_asset};
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// True when this call is a direct `style(...)` call on the imported macro.
    pub(crate) fn is_style_call(&self, call: &CallExpression<'a>) -> bool {
        let Expression::Identifier(identifier) = &call.callee else {
            return false;
        };
        identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            .is_some_and(|symbol| self.style_symbols.contains(&symbol))
    }

    /// Resolves an expression to the style identities it may denote.
    ///
    /// Checker facts win over local knowledge; otherwise an identifier is resolved
    /// through local binding values and a conditional is the union of its branches. The
    /// result is a deduplicated set, so callers must not read the order as source
    /// order. Failure distinguishes a confirmed unsupported value from missing
    /// information about declaration identities.
    pub(crate) fn style_candidates(
        &self,
        expression: &Expression<'a>,
    ) -> Result<Vec<IdentityId>, ResolutionFailure> {
        self.asset_candidates(expression, AssetKind::Style)
    }

    /// Records the expression's range in the AST buffer for a host checker query.
    ///
    /// The selected probe set removes duplicates. When `probing_use` is true, the probe
    /// only asks whether this is a macro receiver, so a missing fact is allowed.
    /// Otherwise, the probe is subject to the required-query policy for lowering.
    ///
    /// Expressions whose syntax proves a non-asset value are skipped, because a fact
    /// for them could only repeat that result. Control-flow expressions handled by
    /// lowering their branches are also skipped.
    pub(crate) fn record_type_probe(&self, expression: &Expression<'a>) {
        // Control-flow expressions are lowered through their branches. A whole-expression
        // fact would bypass branch-specific macro lowering and erase empty alternatives.
        if matches!(
            expression.get_inner_expression(),
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And
        ) || matches!(
            expression.get_inner_expression(),
            Expression::ConditionalExpression(_)
        ) {
            return;
        }
        if cannot_carry_asset(expression) {
            return;
        }
        let span = expression.span();
        let probes = if self.probing_use.get() {
            &self.use_probes
        } else {
            &self.type_probes
        };
        probes.borrow_mut().insert(LocalSpan::from(span));
    }

    /// Lowers one `style()` argument into zero or more IR inputs.
    ///
    /// The order of the attempts is the precedence order of the accepted forms: nested
    /// `style()` calls are reported, then style candidates, then ref attachments,
    /// projections, view transitions, utility templates and finally plain class
    /// expressions.
    pub(crate) fn collect_style_expression(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        let expression = expression.get_inner_expression();
        if let Expression::CallExpression(call) = expression
            && self.is_style_call(call)
        {
            self.omit_input(call.span);
            self.consumed_use_spans.insert(LocalSpan::from(call.span));
            self.consumed_macro_uses
                .push(self.source_span(call.callee.span()));
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::NestedStyleCall,
                self.source_span(call.span),
            ));
            return;
        }
        match expression {
            Expression::ArrayExpression(array) => {
                self.collect_style_array(array, inputs);
                return;
            }
            Expression::ConditionalExpression(conditional) => {
                let mut consequent = Vec::new();
                let mut alternate = Vec::new();
                self.collect_style_expression(&conditional.consequent, &mut consequent);
                self.collect_style_expression(&conditional.alternate, &mut alternate);
                inputs.push(StyleInput::Conditional {
                    test: self.source_span(conditional.test.span()),
                    branches: ConditionalBranches::Ternary {
                        consequent,
                        alternate,
                    },
                });
                return;
            }
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
                let mut consequent = Vec::new();
                self.collect_style_expression(&logical.right, &mut consequent);
                inputs.push(StyleInput::Conditional {
                    test: self.source_span(logical.left.span()),
                    branches: ConditionalBranches::And { consequent },
                });
                return;
            }
            _ => {}
        }
        // A style result with an unknown declaration has a known runtime form, so it is
        // merged as a base. Its CSS has no ordering dependency on this call.
        if let Some(CheckedValue::Asset(ExportedValue::Styles(symbolic))) =
            self.type_fact(expression.span())
            && symbolic.finite_candidates().is_none()
        {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::UnverifiedStyleBase,
                self.source_span(expression.span()),
            ));
            inputs.push(StyleInput::Composition {
                value: self.source_span(expression.span()),
                candidates: Vec::new(),
            });
            return;
        }
        if let Ok(candidates) = self.style_candidates(expression) {
            inputs.push(StyleInput::Composition {
                value: self.source_span(expression.span()),
                candidates,
            });
            return;
        }
        if self.collect_dynamic_ref_attachment(expression, inputs) {
            return;
        }
        if let Some(projection) = self.collect_ref_projection(expression) {
            inputs.push(projection);
            return;
        }
        if let Some(transition) = self.view_transition_use(expression) {
            inputs.push(transition);
            return;
        }
        // A utility template keeps one class expression per utility segment, so it
        // matches a whitespace-separated utility list.
        if let Expression::TemplateLiteral(template) = expression {
            if let Some(expressions) = self.utility_template_expressions(template) {
                inputs.extend(expressions.into_iter().map(StyleInput::Class));
            }
            return;
        }
        if self
            .consumed_use_spans
            .contains(&LocalSpan::from(expression.span()))
        {
            self.omit_input(expression.span());
            return;
        }
        if let Some(class) = self.class_expression(expression) {
            inputs.push(StyleInput::Class(class));
        }
    }

    /// Appends a mixed-channel array to `inputs`, keeping class-only arrays in the
    /// class tree. Empty arrays and elisions append nothing; a nonliteral spread is
    /// diagnosed and skipped.
    fn collect_style_array(&mut self, array: &ArrayExpression<'a>, inputs: &mut Vec<StyleInput>) {
        let mut elements = Vec::new();
        for element in &array.elements {
            match element {
                ArrayExpressionElement::SpreadElement(spread) => {
                    if matches!(
                        spread.argument.get_inner_expression(),
                        Expression::ArrayExpression(_)
                    ) {
                        self.collect_style_expression(&spread.argument, &mut elements);
                    } else {
                        self.omit_input(spread.span);
                        self.lint_findings.push(LintFinding::new(
                            LintDiagnostic::StyleArraySpread,
                            self.source_span(spread.span),
                        ));
                    }
                }
                ArrayExpressionElement::Elision(_) => {}
                _ => {
                    if let Some(expression) = element.as_expression() {
                        self.collect_style_expression(expression, &mut elements);
                    }
                }
            }
        }
        if !elements.is_empty() {
            if let Some(classes) = elements
                .iter()
                .map(class_only_input)
                .collect::<Option<Vec<_>>>()
            {
                inputs.push(StyleInput::Class(ClassExpression::Array(classes)));
            } else {
                inputs.push(StyleInput::Array(elements));
            }
        }
    }

    /// Expands literal argument spreads without changing ordinary argument lowering.
    pub(crate) fn collect_style_spread(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        let Expression::ArrayExpression(array) = expression.get_inner_expression() else {
            self.omit_input(expression.span());
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::StyleArgumentSpread,
                self.source_span(expression.span()),
            ));
            return;
        };
        for element in &array.elements {
            if let ArrayExpressionElement::SpreadElement(spread) = element {
                self.collect_style_spread(&spread.argument, inputs);
            } else if let Some(expression) = element.as_expression() {
                self.collect_style_expression(expression, inputs);
            }
        }
    }
}

/// Returns a class-only subtree when every nested input is a class expression. Returns
/// `None` when a composition, attachment, or root asset input needs another props
/// channel.
fn class_only_input(input: &StyleInput) -> Option<ClassExpression> {
    match input {
        StyleInput::Class(class) => Some(class.clone()),
        StyleInput::Array(inputs) => Some(ClassExpression::Array(
            inputs.iter().map(class_only_input).collect::<Option<_>>()?,
        )),
        StyleInput::Conditional { test, branches } => Some(ClassExpression::Conditional {
            test: *test,
            branches: branches
                .as_ref()
                .try_map(|values| {
                    values
                        .iter()
                        .map(class_only_input)
                        .collect::<Option<_>>()
                        .ok_or(())
                })
                .ok()?,
        }),
        _ => None,
    }
}
