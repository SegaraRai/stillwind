//! Static CVA configuration validation and extraction of independently compiled values.
//!
//! `collect_cva` keeps configuration syntax in the source and records only the callee
//! and style expressions that need replacement. Selection values remain ordinary
//! JavaScript literals. CVA values reuse the style-input parser after syntax validation.

use std::collections::{BTreeMap, BTreeSet};

use oxc::ast::ast::{
    CallExpression, Expression, ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKey,
    PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::{number::ToJsString, operator::LogicalOperator};

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{CvaCall, StyleCall};
use stillwind_source::Spanned;

use crate::{local_span::LocalSpan, state::MacroExtractor};

impl<'a> MacroExtractor<'a> {
    /// Validates one factory and extracts values without creating a reusable style producer.
    /// Invalid syntax records diagnostics; callers must discard the module's output.
    pub(crate) fn collect_cva(&mut self, call: &CallExpression<'a>) {
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.span()));
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.get_inner_expression().span()));
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        let span = self.source_span(call.span);
        if self
            .style_calls
            .iter()
            .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
            || self
                .cva_calls
                .iter()
                .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
        {
            self.diagnostics.push(CompileError::new(
                CompileDiagnostic::InvalidCvaConfig {
                    reason: "nested factories are unsupported".into(),
                },
                span,
            ));
            return;
        }
        if call.optional
            || matches!(call.callee.get_inner_expression(), Expression::StaticMemberExpression(member) if member.optional)
            || call.arguments.len() > 2
            || call
                .arguments
                .iter()
                .any(|argument| argument.as_expression().is_none())
        {
            self.diagnostics.push(CompileError::new(
                CompileDiagnostic::InvalidCvaConfig {
                    reason: "expected an optional base and a static configuration object".into(),
                },
                span,
            ));
            return;
        }
        let mut values = Vec::new();
        if let Some(base) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
        {
            self.cva_value(base, &mut values);
        }
        if let Some(config) = call
            .arguments
            .get(1)
            .and_then(|argument| argument.as_expression())
            && !is_undefined(config)
        {
            self.cva_config(config, &mut values);
        }
        self.cva_calls.push(CvaCall {
            span,
            callee_span: self.source_span(call.callee.span()),
            values,
        });
    }

    fn cva_config(&mut self, expression: &Expression<'a>, values: &mut Vec<StyleCall>) {
        let Some(properties) = self.cva_properties(expression) else {
            return;
        };
        let mut axes = BTreeMap::new();
        for (name, property) in &properties {
            if !matches!(
                name.as_str(),
                "variants" | "defaultVariants" | "compoundVariants"
            ) {
                self.cva_error(&property.value, "unknown configuration property");
            }
            if name == "variants" && !is_undefined(&property.value) {
                let Some(variants) = self.cva_properties(&property.value) else {
                    continue;
                };
                for (axis, choices) in variants {
                    if matches!(axis.as_str(), "class" | "className") {
                        self.cva_error(
                            &choices.value,
                            "class and className are reserved axis names",
                        );
                        continue;
                    }
                    let Some(options) = self.cva_properties(&choices.value) else {
                        continue;
                    };
                    let mut keys = BTreeSet::new();
                    for (key, option) in options {
                        keys.insert(key);
                        self.cva_value(&option.value, values);
                    }
                    axes.insert(axis, keys);
                }
            }
        }
        for (name, property) in properties {
            if is_undefined(&property.value) {
                continue;
            }
            match name.as_str() {
                "defaultVariants" => {
                    if let Some(defaults) = self.cva_properties(&property.value) {
                        for (axis, default) in defaults {
                            self.cva_choice(&axis, &default.value, &axes, false);
                        }
                    }
                }
                "compoundVariants" => {
                    let Expression::ArrayExpression(compounds) =
                        property.value.get_inner_expression()
                    else {
                        self.cva_error(
                            &property.value,
                            "compoundVariants requires a literal array",
                        );
                        continue;
                    };
                    for compound in &compounds.elements {
                        let Some(compound) = compound.as_expression() else {
                            self.diagnostics.push(CompileError::new(
                                CompileDiagnostic::InvalidCvaConfig {
                                    reason:
                                        "compoundVariants cannot contain spreads or empty slots"
                                            .into(),
                                },
                                self.source_span(compound.span()),
                            ));
                            continue;
                        };
                        if let Some(conditions) = self.cva_properties(compound) {
                            for (axis, condition) in conditions {
                                if matches!(axis.as_str(), "class" | "className") {
                                    self.cva_value(&condition.value, values);
                                } else {
                                    self.cva_choice(&axis, &condition.value, &axes, true);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        values.sort_by_key(|value| value.span.start);
    }

    fn cva_properties<'e>(
        &mut self,
        expression: &'e Expression<'a>,
    ) -> Option<Vec<(String, &'e ObjectProperty<'a>)>> {
        let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
            self.cva_error(expression, "expected a literal object");
            return None;
        };
        self.cva_object_properties(object)
    }

    fn cva_object_properties<'e>(
        &mut self,
        object: &'e ObjectExpression<'a>,
    ) -> Option<Vec<(String, &'e ObjectProperty<'a>)>> {
        let mut properties = Vec::new();
        let mut keys = BTreeSet::new();
        for entry in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = entry else {
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidCvaConfig {
                        reason: "objects cannot contain spreads".into(),
                    },
                    self.source_span(entry.span()),
                ));
                return None;
            };
            let name = if let PropertyKey::NumericLiteral(value) = &property.key {
                Some(Spanned::new(
                    value.value.to_js_string(),
                    self.source_span(value.span),
                ))
            } else {
                self.static_property_name(&property.key)
            };
            let Some(name) = name else {
                self.cva_error(&property.value, "keys must be static");
                return None;
            };
            if property.method
                || property.kind != PropertyKind::Init
                || property.computed
                || name.value == "__proto__"
                || !keys.insert(name.value.clone())
            {
                self.cva_error(&property.value, "expected unique plain static properties");
                return None;
            }
            properties.push((name.value, property.as_ref()));
        }
        Some(properties)
    }

    fn cva_choice(
        &mut self,
        axis: &str,
        expression: &Expression<'a>,
        variants: &BTreeMap<String, BTreeSet<String>>,
        multiple: bool,
    ) {
        let Some(options) = variants.get(axis) else {
            self.cva_error(expression, "condition names an unknown variant axis");
            return;
        };
        if multiple && let Expression::ArrayExpression(array) = expression.get_inner_expression() {
            for element in &array.elements {
                if let Some(value) = element.as_expression() {
                    self.cva_choice(axis, value, variants, false);
                } else {
                    self.cva_error(
                        expression,
                        "conditions cannot contain spreads or empty slots",
                    );
                }
            }
            return;
        }
        let key = match expression.get_inner_expression() {
            Expression::StringLiteral(value) => Some(value.value.to_string()),
            Expression::NumericLiteral(value) => Some(value.value.to_js_string()),
            Expression::BooleanLiteral(value) => Some(value.value.to_string()),
            Expression::NullLiteral(_) => None,
            value if is_undefined(value) => None,
            _ => {
                self.cva_error(expression, "conditions must be literal option values");
                return;
            }
        };
        if key.as_ref().is_some_and(|key| !options.contains(key)) {
            self.cva_error(expression, "condition names an unknown variant option");
        }
    }

    fn cva_value(&mut self, expression: &Expression<'a>, values: &mut Vec<StyleCall>) {
        if !self.valid_cva_value(expression) {
            self.diagnostics.push(CompileError::new(
                CompileDiagnostic::InvalidCvaInput,
                self.source_span(expression.span()),
            ));
            return;
        }
        let mut inputs = Vec::new();
        self.in_cva = true;
        self.collect_style_expression(expression, &mut inputs);
        self.in_cva = false;
        values.push(StyleCall {
            span: self.source_span(expression.span()),
            inputs,
        });
    }

    fn valid_cva_value(&self, expression: &Expression<'a>) -> bool {
        match expression.get_inner_expression() {
            Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            Expression::Identifier(_) => is_undefined(expression),
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
                element
                    .as_expression()
                    .is_some_and(|value| self.valid_cva_value(value))
            }),
            Expression::ConditionalExpression(conditional) => {
                self.valid_cva_value(&conditional.consequent)
                    && self.valid_cva_value(&conditional.alternate)
            }
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
                self.valid_cva_value(&logical.right)
            }
            Expression::ObjectExpression(object) => object.properties.iter().all(|entry| {
                let ObjectPropertyKind::ObjectProperty(property) = entry else {
                    return false;
                };
                if property.method || property.kind != PropertyKind::Init {
                    return false;
                }
                let Some(name) = self.static_property_name(&property.key) else {
                    return false;
                };
                name.value.starts_with("--")
                    || ((name.value.is_empty() || name.value.ends_with(':'))
                        && self.valid_cva_value(&property.value))
            }),
            _ => false,
        }
    }

    fn cva_error(&mut self, expression: &Expression<'a>, reason: &str) {
        self.diagnostics.push(CompileError::new(
            CompileDiagnostic::InvalidCvaConfig {
                reason: reason.into(),
            },
            self.source_span(expression.span()),
        ));
    }
}

fn is_undefined(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::Identifier(value) if value.name == "undefined")
}
