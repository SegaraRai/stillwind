//! Lowers class trees and object entries without changing expression evaluation order.

use oxc::ast::ast::{
    ArrayExpressionElement, Expression, ObjectExpression, ObjectProperty, ObjectPropertyKind,
    PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::number::ToJsString;
use oxc::syntax::operator::{LogicalOperator, UnaryOperator};

use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic, LintFinding};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalBranches, CustomPropertyEntry, GroupEntry,
    RuleExpression, StyleInput, StyleVariant,
};
use stillwind_source::Span;

use crate::local_span::LocalSpan;
use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;
use stillwind_semantic::AssetKind;

impl<'a> MacroExtractor<'a> {
    /// Lowers one class-valued expression while mirroring the source structure.
    ///
    /// Returns `None` when the expression contributes no class, such as `null`,
    /// `undefined`, or `false`. Conditional tests remain even when both branches are
    /// empty.
    ///
    /// Literal array spreads are expanded; empty slots contribute no class.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    pub(super) fn class_expression(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<ClassExpression> {
        match expression {
            Expression::StringLiteral(literal) => {
                Some(RuleExpression::Utilities(self.string_literal(literal)).into())
            }
            Expression::TemplateLiteral(template) => {
                let mut expressions = self.utility_template_expressions(template)?;
                match expressions.len() {
                    0 => None,
                    1 => expressions.pop(),
                    _ => Some(ClassExpression::Array(expressions)),
                }
            }
            Expression::LogicalExpression(expression)
                if expression.operator == LogicalOperator::And =>
            {
                let test = self.source_span(expression.left.span());
                let consequent = self.class_expression_sequence(&expression.right);
                Some(ClassExpression::Conditional {
                    test,
                    branches: ConditionalBranches::And { consequent },
                })
            }
            Expression::ConditionalExpression(expression) => {
                let test = self.source_span(expression.test.span());
                let consequent = self.class_expression_sequence(&expression.consequent);
                let alternate = self.class_expression_sequence(&expression.alternate);
                Some(ClassExpression::Conditional {
                    test,
                    branches: ConditionalBranches::Ternary {
                        consequent,
                        alternate,
                    },
                })
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.class_expression(&parenthesized.expression)
            }
            Expression::ArrayExpression(array) => {
                let mut elements = Vec::new();
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            if matches!(
                                spread.argument.get_inner_expression(),
                                Expression::ArrayExpression(_)
                            ) {
                                elements.extend(self.class_expression_sequence(&spread.argument));
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
                            if let Some(expression) = element.as_expression()
                                && let Some(value) = self.class_expression(expression)
                            {
                                elements.push(value);
                            }
                        }
                    }
                }
                if elements.is_empty() {
                    None
                } else {
                    Some(ClassExpression::Array(elements))
                }
            }
            Expression::ObjectExpression(object) => self.object_class_expression(object),
            Expression::NullLiteral(_) => None,
            Expression::BooleanLiteral(value) if !value.value || self.in_cva => None,
            Expression::Identifier(value) if value.name == "undefined" => None,
            Expression::Identifier(_) => {
                let failure = self
                    .asset_candidates(expression, AssetKind::Style)
                    .err()
                    .unwrap_or(ResolutionFailure::Unsupported);
                self.omit_input(expression.span());
                self.report_resolution_failure(
                    expression,
                    failure,
                    LintDiagnostic::InvalidStyleInput,
                    CompileDiagnostic::StyleInputFactsRequired,
                );
                None
            }
            _ => {
                if let Some(StyleInput::Class(class)) = self.collect_ref_projection(expression) {
                    return Some(class);
                }
                if let Some(keyframes) = self.keyframes_use(expression) {
                    return Some(keyframes);
                }
                if let Some(transition) = self.view_transition_use(expression) {
                    return match transition {
                        StyleInput::ViewTransitionUse { transition, name } => {
                            Some(RuleExpression::ViewTransitionUse { transition, name }.into())
                        }
                        StyleInput::DynamicViewTransitionUse {
                            value,
                            transitions,
                            name,
                        } => Some(
                            RuleExpression::DynamicViewTransitionUse {
                                value,
                                transitions,
                                name,
                            }
                            .into(),
                        ),
                        _ => None,
                    };
                }
                if self
                    .consumed_use_spans
                    .contains(&LocalSpan::from(expression.span()))
                {
                    self.omit_input(expression.span());
                    return None;
                }
                let previous_errors = self.diagnostics.len();
                self.report_unconsumed_expression(expression);
                if let Expression::CallExpression(call) = expression
                    && self.asset_candidates(&call.callee, AssetKind::Style)
                        != Err(ResolutionFailure::MissingFacts)
                {
                    self.report_unconsumed_call(call);
                }
                self.omit_input(expression.span());
                if self.diagnostics.len() != previous_errors {
                    return None;
                }
                let failure = self
                    .asset_candidates(expression, AssetKind::Style)
                    .err()
                    .unwrap_or(ResolutionFailure::Unsupported);
                self.report_resolution_failure(
                    expression,
                    failure,
                    LintDiagnostic::InvalidStyleInput,
                    CompileDiagnostic::StyleInputFactsRequired,
                );
                None
            }
        }
    }

    /// Lowers an expression to the sequence of class expressions it contributes.
    ///
    /// Returns zero or one expression in source order. An empty vector means the source
    /// expression contributes no class.
    fn class_expression_sequence(&mut self, expression: &Expression<'a>) -> Vec<ClassExpression> {
        self.class_expression(expression).into_iter().collect()
    }

    /// Lowers a `style({...})` object into attribute entries and wrappers.
    ///
    /// Keys are interpreted by shape: `class` passes a user value through, quoted
    /// `aria-*`/`data-*` keys become attributes, `--*` keys become inline custom
    /// properties, and a key ending in `:` becomes a variant wrapper. The keys `""` and
    /// `":"` group rule inputs without adding variants. Computed keys resolve to
    /// literal names or ref variants. Literal object spreads are expanded with the last
    /// value of each known key.
    fn object_class_expression(
        &mut self,
        object: &ObjectExpression<'a>,
    ) -> Option<ClassExpression> {
        let mut entries = Vec::new();
        let mut properties = Vec::new();
        self.collect_object_properties(object, &mut properties);
        for property in properties {
            if property.method || property.kind != PropertyKind::Init {
                self.omit_input(property.span);
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidAttributeProperty,
                    self.source_span(property.span),
                ));
                continue;
            }
            let name = self.static_property_name(&property.key);
            if property.computed && name.is_none() {
                if let Some(entry) = self.computed_variant_entry(property) {
                    entries.push(entry);
                }
                continue;
            }
            if name.as_ref().is_some_and(|name| name.value == "class") {
                let mut arguments = Vec::new();
                self.collect_class_arguments(&property.value, &mut arguments);
                entries.push(GroupEntry::Class(Box::new(ClassExpression::ClassValue {
                    span: self.source_span(property.value.span()),
                    arguments,
                })));
                continue;
            }
            let Some(name) = name else {
                self.omit_input(property.span);
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidAttributeKey,
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if name.value.starts_with("--") {
                entries.push(GroupEntry::CustomProperty(CustomPropertyEntry {
                    name,
                    value: self.source_span(property.value.span()),
                    static_value: static_css_value(&property.value),
                }));
                continue;
            }
            if !name.value.ends_with(':')
                && (name.value.starts_with("aria-") || name.value.starts_with("data-"))
            {
                entries.push(GroupEntry::Attribute(AttributeEntry {
                    name,
                    value: self.source_span(property.value.span()),
                }));
                continue;
            }
            if name.value.is_empty() || name.value == ":" {
                if let Some(entry) = self.rule_entry(None, &property.value) {
                    entries.push(entry);
                }
                continue;
            }
            if !name.value.ends_with(':') {
                self.omit_input(property.span);
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidAttributeKey,
                    name.span,
                ));
                continue;
            }
            if let Some(entry) = self.rule_entry(Some(StyleVariant::Prefix(name)), &property.value)
            {
                entries.push(entry);
            }
        }
        if entries.is_empty() {
            None
        } else {
            Some(ClassExpression::Group {
                span: self.source_span(object.span),
                entries,
            })
        }
    }

    /// Resolves a computed variant key and omits its property when the key is unusable.
    fn computed_variant_entry(&mut self, property: &ObjectProperty<'a>) -> Option<GroupEntry> {
        let previous_lints = self.lint_findings.len();
        let previous_errors = self.diagnostics.len();
        let Some(variant) = property
            .key
            .as_expression()
            .filter(|key| {
                matches!(
                    key.get_inner_expression(),
                    Expression::CallExpression(_) | Expression::TaggedTemplateExpression(_)
                )
            })
            .and_then(|key| self.ref_variant(key))
        else {
            self.omit_input(property.span);
            if self.lint_findings.len() == previous_lints
                && self.diagnostics.len() == previous_errors
            {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidComputedAttribute,
                    self.source_span(property.key.span()),
                ));
            }
            return None;
        };
        self.rule_entry(Some(variant), &property.value)
    }

    /// Keeps the first key position and last value when literal objects overwrite keys.
    fn collect_object_properties<'b>(
        &mut self,
        object: &'b ObjectExpression<'a>,
        properties: &mut Vec<&'b ObjectProperty<'a>>,
    ) {
        for property in &object.properties {
            match property {
                ObjectPropertyKind::SpreadProperty(spread) => {
                    if let Expression::ObjectExpression(object) =
                        spread.argument.get_inner_expression()
                    {
                        self.collect_object_properties(object, properties);
                    } else {
                        self.omit_input(spread.span);
                        self.lint_findings.push(LintFinding::new(
                            LintDiagnostic::AttributeSpread,
                            self.source_span(spread.span),
                        ));
                    }
                }
                ObjectPropertyKind::ObjectProperty(property) => {
                    let previous = self.static_property_name(&property.key).and_then(|name| {
                        properties.iter().position(|other| {
                            self.static_property_name(&other.key)
                                .is_some_and(|other| other.value == name.value)
                        })
                    });
                    if let Some(index) = previous {
                        properties[index] = property;
                    } else {
                        properties.push(property);
                    }
                }
            }
        }
    }

    /// Expands external class arrays into original-source argument spans.
    /// Spread arrays stay intact because iteration and clsx consumption accept
    /// different values. Guarded arrays remain inside their original expression.
    fn collect_class_arguments(&self, expression: &Expression<'a>, arguments: &mut Vec<Span>) {
        if let Expression::ArrayExpression(array) = expression.get_inner_expression()
            && !array
                .elements
                .iter()
                .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_)))
        {
            for element in &array.elements {
                if let Some(value) = element.as_expression() {
                    self.collect_class_arguments(value, arguments);
                }
            }
        } else {
            arguments.push(self.source_span(expression.span()));
        }
    }
}

/// Preconverts a custom-property literal to the text produced by the CSS-value helper.
///
/// Unwraps source parentheses and type wrappers. Strings and expression-free templates
/// use their decoded text, numbers use JavaScript number formatting, and booleans
/// become one space or `initial`. Unary negation is supported only for a numeric
/// literal operand. Returns `None` for other expressions or an unavailable cooked
/// template value, leaving conversion to runtime even when the expression could be
/// constant.
fn static_css_value(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::StringLiteral(value) => Some(value.value.to_string()),
        Expression::BooleanLiteral(value) => Some(if value.value { " " } else { "initial" }.into()),
        Expression::NumericLiteral(value) => Some(value.value.to_js_string()),
        Expression::UnaryExpression(value) if value.operator == UnaryOperator::UnaryNegation => {
            if let Expression::NumericLiteral(number) = value.argument.get_inner_expression() {
                Some((-number.value).to_js_string())
            } else {
                None
            }
        }
        Expression::TemplateLiteral(value) if value.expressions.is_empty() => value
            .quasis
            .first()?
            .value
            .cooked
            .as_ref()
            .map(ToString::to_string),
        _ => None,
    }
}
