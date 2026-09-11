use oxc::ast::ast::{
    ArrayExpressionElement, Expression, ObjectExpression, ObjectPropertyKind, PropertyKey,
    PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::operator::LogicalOperator;

use stillwind_ir::{AttributeEntry, StyleInput, StyleVariant};
use stillwind_source::{CompileError, Span};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn record_type_probe(&self, expression: &Expression<'a>) {
        if matches!(
            expression,
            Expression::StringLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_)
        ) {
            return;
        }
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
    }

    pub(crate) fn collect_style_expression(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        if self.collect_dynamic_handle_attachment(expression, inputs) {
            return;
        }
        if let Some(projection) = self.collect_handle_projection(expression) {
            inputs.push(projection);
            return;
        }
        if let Some(animation) = self.animation_use(expression) {
            inputs.push(animation.into_style_input());
            return;
        }
        if let Some(transition) = self.view_transition_use(expression) {
            inputs.push(transition);
            return;
        }
        match expression {
            Expression::StringLiteral(literal) => {
                inputs.push(StyleInput::UtilityString(self.string_literal(literal)));
            }
            Expression::TemplateLiteral(template) => self.collect_utility_template(template, inputs),
            Expression::LogicalExpression(expression)
                if expression.operator == LogicalOperator::And =>
            {
                self.collect_guarded_utility(
                    &expression.right, self.source_span(expression.left.span()), true, inputs,
                );
            }
            Expression::ConditionalExpression(expression) => {
                let guard = self.source_span(expression.test.span());
                self.collect_guarded_utility(&expression.consequent, guard, true, inputs);
                self.collect_guarded_utility(&expression.alternate, guard, false, inputs);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => self.diagnostics.push(
                            CompileError::new(
                                "SHJS002",
                                "style() arrays do not support spread elements",
                                self.source_span(spread.span),
                            ),
                        ),
                        ArrayExpressionElement::Elision(elision) => self.diagnostics.push(
                            CompileError::new(
                                "SHJS002",
                                "style() arrays cannot contain empty slots",
                                self.source_span(elision.span),
                            ),
                        ),
                        _ => {
                            if let Some(expression) = element.as_expression() {
                                self.collect_style_expression(expression, inputs);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => self.collect_object(object, inputs),
            Expression::NullLiteral(_) => {},
            Expression::BooleanLiteral(value) if !value.value => {},
            Expression::Identifier(value) if value.name == "undefined" => {},
            Expression::Identifier(_) => self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional handles require TypeScript checker facts",
                self.source_span(expression.span()),
            )),
            _ => self.diagnostics.push(CompileError::new(
                "SHJS002",
                "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and variant wrappers",
                self.source_span(expression.span()),
            )),
        }
    }

    #[allow(clippy::too_many_lines)] // Keeps the tightly coupled object-key diagnostics together.
    pub(crate) fn collect_object(
        &mut self,
        object: &ObjectExpression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        let mut entries = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() attribute objects do not support spread properties",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() attributes must be plain key-value properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            if property.computed {
                let Some(variant) = property
                    .key
                    .as_expression()
                    .and_then(|key| self.ref_variant(key))
                else {
                    self.diagnostics.push(CompileError::new(
                        "SHJS003",
                        "computed style() keys must be ref variants",
                        self.source_span(property.key.span()),
                    ));
                    continue;
                };
                self.collect_variant_wrapper(&property.value, variant, inputs);
                continue;
            }
            if self
                .property_name(&property.key)
                .is_some_and(|name| name.value == "class")
            {
                inputs.push(StyleInput::ExternalClass(
                    self.source_span(property.value.span()),
                ));
                continue;
            }
            let PropertyKey::StringLiteral(name) = &property.key else {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or variant prefixes ending in `:`",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let name = self.string_literal(name);
            if !name.value.ends_with(':')
                && (name.value.starts_with("aria-") || name.value.starts_with("data-"))
            {
                entries.push(AttributeEntry {
                    name,
                    value: self.source_span(property.value.span()),
                });
                continue;
            }
            if !name.value.ends_with(':') || name.value == ":" {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or variant prefixes ending in `:`",
                    name.span,
                ));
                continue;
            }
            self.collect_variant_wrapper(&property.value, StyleVariant::Prefix(name), inputs);
        }
        if !entries.is_empty() {
            inputs.push(StyleInput::Attributes(entries));
        }
    }

    fn collect_guarded_utility(
        &mut self,
        expression: &Expression<'a>,
        guard: Span,
        when_true: bool,
        inputs: &mut Vec<StyleInput>,
    ) {
        let mut nested = Vec::new();
        match expression {
            Expression::StringLiteral(value) => {
                nested.push(StyleInput::UtilityString(self.string_literal(value)));
            }
            Expression::TemplateLiteral(template) => {
                self.collect_utility_template(template, &mut nested);
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    "SHJS002", "a style() conditional must contain literal utility strings or direct utility templates",
                    self.source_span(expression.span()),
                ));
                return;
            }
        }
        for input in &mut nested {
            guard_utility(input, guard, when_true);
        }
        inputs.extend(nested);
    }

    fn collect_variant_wrapper(
        &mut self,
        expression: &Expression<'a>,
        variant: StyleVariant,
        inputs: &mut Vec<StyleInput>,
    ) {
        let mut nested = Vec::new();
        self.collect_style_expression(expression, &mut nested);
        if nested.iter().any(|input| !is_variant_input(input)) {
            self.diagnostics.push(CompileError::new(
                "SHJS004",
                "variant wrappers accept utility strings, animations, arrays, and nested variant wrappers",
                self.source_span(expression.span()),
            ));
            return;
        }
        inputs.push(StyleInput::Variant {
            variant,
            inputs: nested,
        });
    }
}

fn is_variant_input(input: &StyleInput) -> bool {
    match input {
        StyleInput::UtilityString(_)
        | StyleInput::ConditionalUtilityString { .. }
        | StyleInput::AnimationUse { .. }
        | StyleInput::DynamicAnimationUse { .. } => true,
        StyleInput::Variant { inputs, .. } => inputs.iter().all(is_variant_input),
        _ => false,
    }
}

fn guard_utility(input: &mut StyleInput, guard: Span, when_true: bool) {
    match input {
        StyleInput::UtilityString(value) => {
            *input = StyleInput::ConditionalUtilityString {
                guard,
                when_true,
                value: value.clone(),
            };
        }
        StyleInput::Variant { inputs, .. } => {
            for input in inputs {
                guard_utility(input, guard, when_true);
            }
        }
        _ => {}
    }
}
