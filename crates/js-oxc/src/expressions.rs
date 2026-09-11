use oxc::ast::ast::{
    ArrayExpressionElement, CallExpression, Expression, ObjectExpression, ObjectPropertyKind,
    PropertyKey, PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::operator::LogicalOperator;

use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalKind, GroupEntry, StyleInput, StyleVariant,
};
use stillwind_semantic::{ExportedValue, IdentityId};
use stillwind_source::CompileError;

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
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

    pub(crate) fn style_candidates(&self, expression: &Expression<'a>) -> Option<Vec<IdentityId>> {
        self.record_type_probe(expression);
        let span = expression.span();
        if let Some(ExportedValue::Styles(value)) = self.type_facts.get(&(span.start, span.end)) {
            return Some(value.finite_candidates()?.iter().cloned().collect());
        }
        match expression {
            Expression::Identifier(identifier) => {
                let symbol = self
                    .scoping
                    .get_reference(identifier.reference_id.get()?)
                    .symbol_id()?;
                let ExportedValue::Styles(value) = self.local_values.get(&symbol)? else {
                    return None;
                };
                Some(value.finite_candidates()?.iter().cloned().collect())
            }
            Expression::ConditionalExpression(conditional) => {
                let mut candidates = self.style_candidates(&conditional.consequent)?;
                candidates.extend(self.style_candidates(&conditional.alternate)?);
                candidates.sort();
                candidates.dedup();
                Some(candidates)
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.style_candidates(&parenthesized.expression)
            }
            _ => None,
        }
    }

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
        if let Expression::CallExpression(call) = expression
            && self.is_style_call(call)
        {
            self.diagnostics.push(CompileError::new(
                "SHCOMP001",
                "nested style() calls are not supported; bind the base to a variable",
                self.source_span(call.span),
            ));
            return;
        }
        if let Some(candidates) = self.style_candidates(expression) {
            inputs.push(StyleInput::Composition {
                value: self.source_span(expression.span()),
                candidates,
            });
            return;
        }
        if self.collect_dynamic_handle_attachment(expression, inputs) {
            return;
        }
        if let Some(projection) = self.collect_handle_projection(expression) {
            inputs.push(projection);
            return;
        }
        if let Some(transition) = self.view_transition_use(expression) {
            inputs.push(transition);
            return;
        }
        // A utility template keeps one class expression per utility segment, so
        // it matches a whitespace-separated utility list.
        if let Expression::TemplateLiteral(template) = expression
            && let Some(expressions) = self.utility_template_expressions(template)
        {
            inputs.extend(expressions.into_iter().map(StyleInput::Class));
            return;
        }
        if let Some(class) = self.class_expression(expression) {
            inputs.push(StyleInput::Class(class));
        }
    }

    /// Lowers one class-valued expression while mirroring the source structure.
    ///
    /// Returns `None` when the expression contributes no class, such as
    /// `null`, `undefined`, or `false`.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    fn class_expression(&mut self, expression: &Expression<'a>) -> Option<ClassExpression> {
        match expression {
            Expression::StringLiteral(literal) => {
                Some(ClassExpression::Utilities(self.string_literal(literal)))
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
                if consequent.is_empty() {
                    return None;
                }
                Some(ClassExpression::Conditional {
                    test,
                    kind: ConditionalKind::LogicalAnd,
                    consequent,
                    alternate: Vec::new(),
                })
            }
            Expression::ConditionalExpression(expression) => {
                let test = self.source_span(expression.test.span());
                let consequent = self.class_expression_sequence(&expression.consequent);
                let alternate = self.class_expression_sequence(&expression.alternate);
                if consequent.is_empty() && alternate.is_empty() {
                    return None;
                }
                Some(ClassExpression::Conditional {
                    test,
                    kind: ConditionalKind::Ternary,
                    consequent,
                    alternate,
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
                            self.diagnostics.push(CompileError::new(
                                "SHJS002",
                                "style() arrays do not support spread elements",
                                self.source_span(spread.span),
                            ));
                        }
                        ArrayExpressionElement::Elision(elision) => {
                            self.diagnostics.push(CompileError::new(
                                "SHJS002",
                                "style() arrays cannot contain empty slots",
                                self.source_span(elision.span),
                            ));
                        }
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
            Expression::BooleanLiteral(value) if !value.value => None,
            Expression::Identifier(value) if value.name == "undefined" => None,
            Expression::Identifier(_) => {
                self.diagnostics.push(CompileError::new(
                    "SHTS001",
                    "non-local or conditional handles require TypeScript checker facts",
                    self.source_span(expression.span()),
                ));
                None
            }
            _ => {
                if let Some(StyleInput::Class(class)) = self.collect_handle_projection(expression) {
                    return Some(class);
                }
                if let Some(animation) = self.animation_use(expression) {
                    return Some(animation.into_class_expression());
                }
                self.diagnostics.push(CompileError::new(
                    "SHJS002",
                    "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and variant wrappers",
                    self.source_span(expression.span()),
                ));
                None
            }
        }
    }

    fn class_expression_sequence(&mut self, expression: &Expression<'a>) -> Vec<ClassExpression> {
        self.class_expression(expression).into_iter().collect()
    }

    fn object_class_expression(
        &mut self,
        object: &ObjectExpression<'a>,
    ) -> Option<ClassExpression> {
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
                if let Some(entry) = self.variant_entry(variant, &property.value) {
                    entries.push(entry);
                }
                continue;
            }
            if self
                .property_name(&property.key)
                .is_some_and(|name| name.value == "class")
            {
                entries.push(GroupEntry::Class(Box::new(ClassExpression::External(
                    self.source_span(property.value.span()),
                ))));
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
                entries.push(GroupEntry::Attribute(AttributeEntry {
                    name,
                    value: self.source_span(property.value.span()),
                }));
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
            if let Some(entry) = self.variant_entry(StyleVariant::Prefix(name), &property.value) {
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

    fn variant_entry(
        &mut self,
        variant: StyleVariant,
        value: &Expression<'a>,
    ) -> Option<GroupEntry> {
        let inputs = self.class_expression_sequence(value);
        if inputs.is_empty() {
            return None;
        }
        if !inputs.iter().all(variant_compatible) {
            self.diagnostics.push(CompileError::new(
                "SHJS004",
                "variant wrappers accept utility strings, animations, arrays, and nested variant wrappers",
                self.source_span(value.span()),
            ));
            return None;
        }
        Some(GroupEntry::Class(Box::new(ClassExpression::Variant {
            variant,
            inputs,
        })))
    }
}

/// Variant wrappers scope class declarations only; attributes and user class
/// values cannot be variant-scoped.
fn variant_compatible(expression: &ClassExpression) -> bool {
    match expression {
        ClassExpression::Utilities(_)
        | ClassExpression::AnimationUse { .. }
        | ClassExpression::DynamicAnimationUse { .. } => true,
        ClassExpression::Array(elements) => elements.iter().all(variant_compatible),
        ClassExpression::Conditional {
            consequent,
            alternate,
            ..
        } => consequent
            .iter()
            .chain(alternate.iter())
            .all(variant_compatible),
        ClassExpression::Variant { inputs, .. } => inputs.iter().all(variant_compatible),
        ClassExpression::Group { entries, .. } => entries.iter().all(|entry| match entry {
            GroupEntry::Attribute(_) => false,
            GroupEntry::Class(expression) => variant_compatible(expression),
        }),
        ClassExpression::External(_)
        | ClassExpression::HandleProjection { .. }
        | ClassExpression::DynamicHandleProjection { .. } => false,
    }
}
