//! Static CVA configuration validation and extraction of independently compiled values.
//!
//! `collect_cva` keeps configuration syntax in the source and records only the callee
//! and style expressions that need replacement. Selection values remain ordinary
//! JavaScript literals. CVA values reuse the style-input parser after syntax
//! validation.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    ArrayExpressionElement, CallExpression, Expression, ObjectExpression, ObjectProperty,
    ObjectPropertyKind, PropertyKey, PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::number::ToJsString;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{CvaCall, CvaReplacement, OutputFormat, StyleCall};
use stillwind_source::Spanned;

use crate::{local_span::LocalSpan, state::MacroExtractor};

impl<'a> MacroExtractor<'a> {
    /// Validates one factory and extracts values without creating a reusable style
    /// producer. Unsupported values contribute nothing. Invalid configuration entries
    /// are replaced with empty entries, and invalid conditions omit their whole
    /// compound.
    pub(crate) fn collect_cva(&mut self, call: &CallExpression<'a>, output: OutputFormat) {
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
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidCvaConfig {
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
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidCvaConfig {
                    reason: "expected an optional base and a static configuration object".into(),
                },
                span,
            ));
            self.omit_input(call.span);
            self.cva_calls.push(CvaCall {
                span,
                output,
                callee_span: self.source_span(call.callee.span()),
                values: Vec::new(),
                empty: true,
                replacements: Vec::new(),
            });
            return;
        }
        let replacement_start = self.cva_replacements.len();
        let mut values = Vec::new();
        if let Some(base) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
        {
            self.cva_value(base, output, &mut values);
        }
        if let Some(config) = call
            .arguments
            .get(1)
            .and_then(|argument| argument.as_expression())
            && !is_undefined(config)
        {
            self.cva_config(config, output, &mut values);
        }
        self.cva_calls.push(CvaCall {
            span,
            output,
            callee_span: self.source_span(call.callee.span()),
            values,
            empty: false,
            replacements: self.cva_replacements.split_off(replacement_start),
        });
    }

    fn cva_config(
        &mut self,
        expression: &Expression<'a>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
    ) {
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
                self.cva_omit_property(property);
                continue;
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
                        self.cva_omit_property(choices);
                        continue;
                    }
                    let Some(options) = self.cva_properties(&choices.value) else {
                        continue;
                    };
                    let mut keys = BTreeSet::new();
                    for (key, option) in options {
                        keys.insert(key);
                        self.cva_value(&option.value, output, values);
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
                            if !self.cva_choice(&axis, &default.value, &axes, false) {
                                self.cva_omit_property(default);
                            }
                        }
                    }
                }
                "compoundVariants" => {
                    self.cva_compounds(&property.value, &axes, output, values);
                }
                _ => {}
            }
        }
        values.sort_by_key(|value| value.span.start);
    }

    fn cva_compounds(
        &mut self,
        expression: &Expression<'a>,
        axes: &BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
    ) {
        let Expression::ArrayExpression(compounds) = expression.get_inner_expression() else {
            self.cva_error(expression, "compoundVariants requires a literal array");
            self.cva_replace(expression, "[]");
            return;
        };
        for compound in &compounds.elements {
            let Some(compound) = compound.as_expression() else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidCvaConfig {
                        reason: "compoundVariants cannot contain spreads or empty slots".into(),
                    },
                    self.source_span(compound.span()),
                ));
                self.omit_input(compound.span());
                self.cva_replacements.push(CvaReplacement {
                    span: self.source_span(compound.span()),
                    replacement: if matches!(compound, ArrayExpressionElement::Elision(_)) {
                        "...[],"
                    } else {
                        "...[]"
                    }
                    .into(),
                });
                continue;
            };
            let finding_start = self.lint_findings.len();
            let value_start = values.len();
            let edit_start = self.cva_replacements.len();
            let mut valid = true;
            if let Some(conditions) = self.cva_properties(compound) {
                for (axis, condition) in conditions {
                    if matches!(axis.as_str(), "class" | "className") {
                        self.cva_value(&condition.value, output, values);
                    } else {
                        valid &= self.cva_choice(&axis, &condition.value, axes, true);
                    }
                }
            } else {
                valid = false;
            }
            valid &= !self.lint_findings[finding_start..].iter().any(|finding| {
                matches!(finding.diagnostic, LintDiagnostic::InvalidCvaConfig { .. })
            });
            if !valid {
                values.truncate(value_start);
                self.cva_replacements.truncate(edit_start);
                self.cva_replace(compound, "...[]");
            }
        }
    }

    fn cva_properties<'e>(
        &mut self,
        expression: &'e Expression<'a>,
    ) -> Option<Vec<(LeanStr, &'e ObjectProperty<'a>)>> {
        let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
            self.cva_error(expression, "expected a literal object");
            self.cva_replace(expression, "{}");
            return None;
        };
        Some(self.cva_object_properties(object))
    }

    fn cva_object_properties<'e>(
        &mut self,
        object: &'e ObjectExpression<'a>,
    ) -> Vec<(LeanStr, &'e ObjectProperty<'a>)> {
        let mut properties = Vec::new();
        let mut keys = BTreeSet::new();
        for entry in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = entry else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidCvaConfig {
                        reason: "objects cannot contain spreads".into(),
                    },
                    self.source_span(entry.span()),
                ));
                self.omit_input(entry.span());
                self.cva_replacements.push(CvaReplacement {
                    span: self.source_span(entry.span()),
                    replacement: "...{}".into(),
                });
                continue;
            };
            let name = if let PropertyKey::NumericLiteral(value) = &property.key {
                Some(Spanned::new(
                    value.value.to_js_string().to_lean_str(),
                    self.source_span(value.span),
                ))
            } else {
                self.static_property_name(&property.key)
            };
            let Some(name) = name else {
                self.cva_error(&property.value, "keys must be static");
                self.cva_omit_property(property);
                continue;
            };
            if property.method
                || property.kind != PropertyKind::Init
                || property.computed
                || name.value == "__proto__"
                || !keys.insert(name.value.clone())
            {
                self.cva_error(&property.value, "expected unique plain static properties");
                self.cva_omit_property(property);
                continue;
            }
            properties.push((name.value, property.as_ref()));
        }
        properties
    }

    fn cva_choice(
        &mut self,
        axis: &str,
        expression: &Expression<'a>,
        variants: &BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        multiple: bool,
    ) -> bool {
        let Some(options) = variants.get(axis) else {
            self.cva_error(expression, "condition names an unknown variant axis");
            return false;
        };
        if multiple && let Expression::ArrayExpression(array) = expression.get_inner_expression() {
            let mut valid = true;
            for element in &array.elements {
                if let Some(value) = element.as_expression() {
                    valid &= self.cva_choice(axis, value, variants, false);
                } else {
                    self.cva_error(
                        expression,
                        "conditions cannot contain spreads or empty slots",
                    );
                    valid = false;
                }
            }
            return valid;
        }
        let key = match expression.get_inner_expression() {
            Expression::StringLiteral(value) => self
                .unicode_string(value.value, value.span)
                .map(ToLeanStr::to_lean_str),
            Expression::NumericLiteral(value) => Some(value.value.to_js_string().to_lean_str()),
            Expression::BooleanLiteral(value) => Some(value.value.to_lean_str()),
            Expression::NullLiteral(_) => None,
            value if is_undefined(value) => None,
            _ => {
                self.cva_error(expression, "conditions must be literal option values");
                return false;
            }
        };
        if key.as_ref().is_some_and(|key| !options.contains(key)) {
            self.cva_error(expression, "condition names an unknown variant option");
            return false;
        }
        true
    }

    fn cva_value(
        &mut self,
        expression: &Expression<'a>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
    ) {
        if !self.valid_cva_value(expression) {
            self.omit_input(expression.span());
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidCvaInput,
                self.source_span(expression.span()),
            ));
            values.push(StyleCall {
                span: self.source_span(expression.span()),
                output,
                inputs: Vec::new(),
            });
            return;
        }
        let mut inputs = Vec::new();
        self.in_cva = true;
        self.collect_style_expression(expression, &mut inputs);
        self.in_cva = false;
        values.push(StyleCall {
            span: self.source_span(expression.span()),
            output,
            inputs,
        });
    }

    fn valid_cva_value(&mut self, expression: &Expression<'a>) -> bool {
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

    fn cva_replace(&mut self, expression: &Expression<'a>, replacement: &str) {
        self.omit_input(expression.span());
        self.cva_replacements.push(CvaReplacement {
            span: self.source_span(expression.span()),
            replacement: replacement.into(),
        });
    }

    fn cva_omit_property(&mut self, property: &ObjectProperty<'a>) {
        self.omit_input(property.span);
        self.cva_replacements.push(CvaReplacement {
            span: self.source_span(property.span),
            replacement: "...{}".into(),
        });
    }

    fn cva_error(&mut self, expression: &Expression<'a>, reason: &str) {
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidCvaConfig {
                reason: reason.into(),
            },
            self.source_span(expression.span()),
        ));
    }
}

fn is_undefined(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::Identifier(value) if value.name == "undefined")
}
