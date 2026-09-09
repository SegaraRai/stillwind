use super::{
    ArrayExpressionElement, AttributeEntry, CompileError, Expression, GetSpan, LogicalOperator,
    MacroExtractor, ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, StyleInput,
};

impl<'a> MacroExtractor<'a> {
    pub(super) fn collect_style_expression(
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
            inputs.push(animation);
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
            Expression::LogicalExpression(expression)
                if expression.operator == LogicalOperator::And =>
            {
                let Expression::StringLiteral(literal) = &expression.right else {
                    self.diagnostics.push(CompileError::new(
                        "SHJS002",
                        "a style() conditional must have a literal utility string on its right side",
                        self.source_span(expression.right.span()),
                    ));
                    return;
                };
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard: self.source_span(expression.left.span()),
                    when_true: true,
                    value: self.string_literal(literal),
                });
            }
            Expression::ConditionalExpression(expression) => {
                let (Expression::StringLiteral(consequent), Expression::StringLiteral(alternate)) =
                    (&expression.consequent, &expression.alternate)
                else {
                    self.diagnostics.push(CompileError::new(
                        "SHJS002",
                        "a style() ternary must contain literal utility strings on both sides",
                        self.source_span(expression.span),
                    ));
                    return;
                };
                let guard = self.source_span(expression.test.span());
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard,
                    when_true: true,
                    value: self.string_literal(consequent),
                });
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard,
                    when_true: false,
                    value: self.string_literal(alternate),
                });
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
                        _ => self.collect_style_expression(element.to_expression(), inputs),
                    }
                }
            }
            Expression::ObjectExpression(object) => self.collect_object(object, inputs),
            Expression::Identifier(_) => self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional handles require TypeScript checker facts",
                self.source_span(expression.span()),
            )),
            _ => self.diagnostics.push(CompileError::new(
                "SHJS002",
                "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and `&` selectors",
                self.source_span(expression.span()),
            )),
        }
    }

    #[allow(clippy::too_many_lines)] // Keeps the tightly coupled object-key diagnostics together.
    pub(super) fn collect_object(
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
                let Some((handles, selector)) = self.handle_tagged_selector(&property.key) else {
                    self.diagnostics.push(CompileError::new(
                        "SHJS003",
                        "computed style() keys must be static selectors tagged by style.ref()",
                        self.source_span(property.key.span()),
                    ));
                    continue;
                };
                match &property.value {
                    Expression::StringLiteral(value) => {
                        for handle in handles {
                            inputs.push(StyleInput::HandleSelectorUtilityString {
                                handle,
                                selector: selector.clone(),
                                value: self.string_literal(value),
                            });
                        }
                    }
                    Expression::LogicalExpression(expression)
                        if expression.operator == LogicalOperator::And =>
                    {
                        let Expression::StringLiteral(value) = &expression.right else {
                            self.diagnostics.push(CompileError::new(
                                "SHJS004",
                                "a ref selector conditional must have a literal utility string on its right side",
                                self.source_span(expression.right.span()),
                            ));
                            continue;
                        };
                        for handle in handles {
                            inputs.push(StyleInput::ConditionalHandleSelectorUtilityString {
                                handle,
                                selector: selector.clone(),
                                guard: self.source_span(expression.left.span()),
                                when_true: true,
                                value: self.string_literal(value),
                            });
                        }
                    }
                    _ => self.diagnostics.push(CompileError::new(
                        "SHJS004",
                        "a ref selector must map to a literal utility string or `condition && \"…\"`",
                        self.source_span(property.value.span()),
                    )),
                }
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
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or `&` selectors",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let name = self.string_literal(name);
            if name.value.starts_with("aria-") || name.value.starts_with("data-") {
                entries.push(AttributeEntry {
                    name,
                    value: self.source_span(property.value.span()),
                });
                continue;
            }
            if !name.value.starts_with('&') {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or `&` selectors",
                    name.span,
                ));
                continue;
            }
            match &property.value {
                Expression::CallExpression(_) => {
                    let Some(animation) = self.animation_use(&property.value) else {
                        self.diagnostics.push(CompileError::new(
                            "SHJS004",
                            "a `&` selector must map to a literal utility string, animation use, or `condition && \"…\"`",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    match animation {
                        StyleInput::AnimationUse { animation, options } => {
                            inputs.push(StyleInput::SelectorAnimationUse {
                                selector: name,
                                animation,
                                options,
                            });
                        }
                        StyleInput::DynamicAnimationUse {
                            value,
                            animations,
                            options,
                        } => inputs.push(StyleInput::DynamicSelectorAnimationUse {
                            selector: name,
                            value,
                            animations,
                            options,
                        }),
                        _ => unreachable!("animation_use returns an animation input"),
                    }
                }
                Expression::StringLiteral(value) => {
                    inputs.push(StyleInput::SelectorUtilityString {
                        selector: name,
                        value: self.string_literal(value),
                    });
                }
                Expression::LogicalExpression(expression)
                    if expression.operator == LogicalOperator::And =>
                {
                    let Expression::StringLiteral(value) = &expression.right else {
                        self.diagnostics.push(CompileError::new(
                            "SHJS004",
                            "a selector conditional must have a literal utility string on its right side",
                            self.source_span(expression.right.span()),
                        ));
                        continue;
                    };
                    inputs.push(StyleInput::ConditionalSelectorUtilityString {
                        selector: name,
                        guard: self.source_span(expression.left.span()),
                        when_true: true,
                        value: self.string_literal(value),
                    });
                }
                _ => self.diagnostics.push(CompileError::new(
                    "SHJS004",
                    "a `&` selector must map to a literal utility string or `condition && \"…\"`",
                    self.source_span(property.value.span()),
                )),
            }
        }
        if !entries.is_empty() {
            inputs.push(StyleInput::Attributes(entries));
        }
    }
}
