//! Static recipe configuration validation and extraction of independently compiled
//! values.
//!
//! [`MacroExtractor::collect_recipe`] keeps configuration syntax in the source and
//! records only the callee and style expressions that need replacement. Selection values
//! remain ordinary JavaScript literals. Recipe values reuse the style-input parser after
//! syntax validation.
//!
//! A configuration with `slots` names element parts. Option values and compound
//! `class` and `className` values may then be objects whose keys all name slots,
//! and `compoundSlots` entries add classes to several slots. The values inside such
//! objects are extracted as separate recipe values.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    ArrayExpressionElement, CallExpression, Expression, ObjectExpression, ObjectProperty,
    ObjectPropertyKind, PropertyKey, PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::number::ToJsString;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{
    OutputFormat, RecipeCall, RecipeCondition, RecipeDefaultVariant, RecipePreviewScope,
    RecipeReplacement, RecipeValueRole, StyleCall,
};
use stillwind_source::Spanned;

use crate::{local_span::LocalSpan, state::MacroExtractor};

impl<'a> MacroExtractor<'a> {
    /// Validates one factory and extracts values without creating a reusable style
    /// producer. Unsupported values contribute nothing. Invalid configuration entries
    /// are replaced with empty entries, and invalid conditions omit their whole
    /// compound.
    pub(crate) fn collect_recipe(&mut self, call: &CallExpression<'a>, output: OutputFormat) {
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
                .recipe_calls
                .iter()
                .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
        {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidRecipeConfig {
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
                LintDiagnostic::InvalidRecipeConfig {
                    reason: "expected an optional base and a static configuration object".into(),
                },
                span,
            ));
            self.omit_input(call.span);
            self.recipe_calls.push(RecipeCall {
                span,
                output,
                callee_span: self.source_span(call.callee.span()),
                values: Vec::new(),
                empty: true,
                replacements: Vec::new(),
                roles: Vec::new(),
                has_slots: false,
                default_variants: Vec::new(),
                preview_scopes: Vec::new(),
            });
            return;
        }
        let replacement_start = self.recipe_replacements.len();
        let role_start = self.recipe_roles.len();
        let mut values = Vec::new();
        let mut has_slots = false;
        let mut default_variants = Vec::new();
        let mut preview_scopes = Vec::new();
        if let Some(base) = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
        {
            self.recipe_value(
                base,
                output,
                &mut values,
                &[LeanStr::from("base")],
                false,
                &[],
            );
        }
        if let Some(config) = call
            .arguments
            .get(1)
            .and_then(|argument| argument.as_expression())
            && !is_undefined(config)
        {
            has_slots = self.recipe_config(
                config,
                output,
                &mut values,
                &mut default_variants,
                &mut preview_scopes,
            );
        }
        self.recipe_calls.push(RecipeCall {
            span,
            output,
            callee_span: self.source_span(call.callee.span()),
            values,
            empty: false,
            replacements: self.recipe_replacements.split_off(replacement_start),
            roles: self.recipe_roles.split_off(role_start),
            has_slots,
            default_variants,
            preview_scopes,
        });
    }

    fn recipe_config(
        &mut self,
        expression: &Expression<'a>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        default_variants: &mut Vec<RecipeDefaultVariant>,
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) -> bool {
        let Some(properties) = self.recipe_properties(expression) else {
            return false;
        };
        let slots = properties
            .iter()
            .find(|(name, property)| name == "slots" && !is_undefined(&property.value))
            .and_then(|(_, property)| {
                self.recipe_slots(&property.value, output, values, preview_scopes)
            });
        let mut axes = BTreeMap::new();
        for (name, property) in &properties {
            if !matches!(
                name.as_str(),
                "slots" | "variants" | "defaultVariants" | "compoundVariants" | "compoundSlots"
            ) {
                self.recipe_error(&property.value, "unknown configuration property");
                self.recipe_omit_property(property);
                continue;
            }
            if name == "variants" && !is_undefined(&property.value) {
                self.recipe_variants(
                    &property.value,
                    slots.as_ref(),
                    output,
                    values,
                    &mut axes,
                    preview_scopes,
                );
            }
        }
        for (name, property) in properties {
            if is_undefined(&property.value) {
                continue;
            }
            match name.as_str() {
                "defaultVariants" => {
                    if let Some(defaults) = self.recipe_properties(&property.value) {
                        for (axis, default) in defaults {
                            if let Some(keys) =
                                self.recipe_choice(&axis, &default.value, &axes, false)
                            {
                                if let Some(value) = keys.into_iter().next() {
                                    default_variants.push(RecipeDefaultVariant { axis, value });
                                }
                            } else {
                                self.recipe_omit_property(default);
                            }
                        }
                    }
                }
                "compoundVariants" => {
                    self.recipe_compounds(
                        &property.value,
                        &axes,
                        slots.as_ref(),
                        output,
                        values,
                        preview_scopes,
                    );
                }
                "compoundSlots" => {
                    if let Some(slots) = &slots {
                        self.recipe_compound_slots(
                            &property.value,
                            &axes,
                            slots,
                            output,
                            values,
                            preview_scopes,
                        );
                    } else {
                        self.recipe_error(&property.value, "compoundSlots requires slots");
                        self.recipe_omit_property(property);
                    }
                }
                _ => {}
            }
        }
        values.sort_by_key(|value| value.span.start);
        slots.is_some()
    }

    fn recipe_variants(
        &mut self,
        expression: &Expression<'a>,
        slots: Option<&BTreeSet<LeanStr>>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        axes: &mut BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) {
        let Some(variants) = self.recipe_properties(expression) else {
            return;
        };
        for (axis, choices) in variants {
            if matches!(axis.as_str(), "class" | "className") {
                self.recipe_error(
                    &choices.value,
                    "class and className are reserved axis names",
                );
                self.recipe_omit_property(choices);
                continue;
            }
            let Some(options) = self.recipe_properties(&choices.value) else {
                continue;
            };
            let axis_value_start = values.len();
            let mut keys = BTreeSet::new();
            for (key, option) in options {
                keys.insert(key.clone());
                let option_value_start = values.len();
                let condition = RecipeCondition {
                    axis: axis.clone(),
                    values: vec![key],
                };
                self.recipe_slot_value(
                    &option.value,
                    slots,
                    output,
                    values,
                    std::slice::from_ref(&condition),
                    preview_scopes,
                );
                let option_values: Vec<_> = values[option_value_start..]
                    .iter()
                    .map(|v| v.span)
                    .collect();
                if !option_values.is_empty() {
                    preview_scopes.push(RecipePreviewScope {
                        span: self.source_span(option.span),
                        values: option_values,
                    });
                }
            }
            let axis_values: Vec<_> = values[axis_value_start..].iter().map(|v| v.span).collect();
            if !axis_values.is_empty() {
                preview_scopes.push(RecipePreviewScope {
                    span: self.source_span(choices.span),
                    values: axis_values,
                });
            }
            axes.insert(axis, keys);
        }
    }

    /// Extracts the value of each slot and returns the slot names, including `base`.
    /// A name that is empty, starts with `--`, ends with `:`, or is `class` or
    /// `className` is reported and omitted, because an option value could not tell
    /// such a slot map from a utility input or from the class props.
    fn recipe_slots(
        &mut self,
        expression: &Expression<'a>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) -> Option<BTreeSet<LeanStr>> {
        let properties = self.recipe_properties(expression)?;
        let mut names = BTreeSet::from([LeanStr::from("base")]);
        for (name, property) in properties {
            if !is_slot_name(&name) {
                self.recipe_error(&property.value, "invalid slot name");
                self.recipe_omit_property(property);
                continue;
            }
            let value_start = values.len();
            self.recipe_value(
                &property.value,
                output,
                values,
                std::slice::from_ref(&name),
                false,
                &[],
            );
            let slot_values: Vec<_> = values[value_start..].iter().map(|v| v.span).collect();
            if !slot_values.is_empty() {
                preview_scopes.push(RecipePreviewScope {
                    span: self.source_span(property.span),
                    values: slot_values,
                });
            }
            names.insert(name);
        }
        Some(names)
    }

    /// Extracts a value that applies to the `base` slot, or the values of an object
    /// whose keys all name slots. An unknown slot is reported and its entry omitted.
    fn recipe_slot_value(
        &mut self,
        expression: &Expression<'a>,
        slots: Option<&BTreeSet<LeanStr>>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        conditions: &[RecipeCondition],
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) {
        let (slots, object) = match (slots, expression.get_inner_expression()) {
            (Some(slots), Expression::ObjectExpression(object))
                if !object.properties.is_empty()
                    && object.properties.iter().all(|entry| {
                        matches!(entry, ObjectPropertyKind::ObjectProperty(property)
                            if !property.computed
                                && self
                                    .static_property_name(&property.key)
                                    .is_some_and(|name| is_slot_name(&name.value)))
                    }) =>
            {
                (slots, object)
            }
            _ => {
                self.recipe_value(
                    expression,
                    output,
                    values,
                    &[LeanStr::from("base")],
                    true,
                    conditions,
                );
                return;
            }
        };
        for (name, property) in self.recipe_object_properties(object) {
            if slots.contains(&name) {
                let value_start = values.len();
                self.recipe_value(
                    &property.value,
                    output,
                    values,
                    std::slice::from_ref(&name),
                    true,
                    conditions,
                );
                let slot_values: Vec<_> = values[value_start..].iter().map(|v| v.span).collect();
                if !slot_values.is_empty() {
                    preview_scopes.push(RecipePreviewScope {
                        span: self.source_span(property.span),
                        values: slot_values,
                    });
                }
            } else {
                self.recipe_error(&property.value, "unknown slot");
                self.recipe_omit_property(property);
            }
        }
    }

    fn recipe_compound_slots(
        &mut self,
        expression: &Expression<'a>,
        axes: &BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        slots: &BTreeSet<LeanStr>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) {
        let Expression::ArrayExpression(entries) = expression.get_inner_expression() else {
            self.recipe_error(expression, "compoundSlots requires a literal array");
            self.recipe_replace(expression, "[]");
            return;
        };
        for entry in &entries.elements {
            let Some(entry) = entry.as_expression() else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidRecipeConfig {
                        reason: "compoundSlots cannot contain spreads or empty slots".into(),
                    },
                    self.source_span(entry.span()),
                ));
                self.omit_input(entry.span());
                self.recipe_replacements.push(RecipeReplacement {
                    span: self.source_span(entry.span()),
                    replacement: if matches!(entry, ArrayExpressionElement::Elision(_)) {
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
            let edit_start = self.recipe_replacements.len();
            let role_start = self.recipe_roles.len();
            let mut valid = true;
            let mut entry_conditions = Vec::new();
            if let Some(properties) = self.recipe_properties(entry) {
                let mut listed = None;
                for (key, property) in properties {
                    match key.as_str() {
                        "class" | "className" => {
                            self.recipe_value(&property.value, output, values, &[], true, &[]);
                        }
                        "slots" => {
                            let names = self.recipe_slot_list(&property.value, slots);
                            valid &= names.is_some();
                            listed = Some(names.unwrap_or_default());
                        }
                        _ => {
                            if let Some(keys) =
                                self.recipe_choice(&key, &property.value, axes, true)
                            {
                                if !keys.is_empty() {
                                    entry_conditions.push(RecipeCondition {
                                        axis: key,
                                        values: keys,
                                    });
                                }
                            } else {
                                valid = false;
                            }
                        }
                    }
                }
                if let Some(names) = listed {
                    for role in &mut self.recipe_roles[role_start..] {
                        role.slots.clone_from(&names);
                        role.conditions.clone_from(&entry_conditions);
                    }
                } else {
                    self.recipe_error(entry, "compoundSlots entries require slots");
                }
            } else {
                valid = false;
            }
            valid &= !self.lint_findings[finding_start..].iter().any(|finding| {
                matches!(
                    finding.diagnostic,
                    LintDiagnostic::InvalidRecipeConfig { .. }
                )
            });
            if valid {
                let entry_values: Vec<_> = values[value_start..].iter().map(|v| v.span).collect();
                if !entry_values.is_empty() {
                    preview_scopes.push(RecipePreviewScope {
                        span: self.source_span(entry.span()),
                        values: entry_values,
                    });
                }
            } else {
                values.truncate(value_start);
                self.recipe_roles.truncate(role_start);
                self.recipe_replacements.truncate(edit_start);
                self.recipe_replace(entry, "...[]");
            }
        }
    }

    /// Returns the slot names of `expression` in written order when it is a literal
    /// array of declared slot names. Reports it and returns `None` otherwise.
    fn recipe_slot_list(
        &mut self,
        expression: &Expression<'a>,
        slots: &BTreeSet<LeanStr>,
    ) -> Option<Vec<LeanStr>> {
        let names = match expression.get_inner_expression() {
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .map(|element| {
                    match element
                        .as_expression()
                        .map(Expression::get_inner_expression)
                    {
                        Some(Expression::StringLiteral(value)) => self
                            .unicode_string(value.value, value.span)
                            .filter(|name| slots.contains(*name))
                            .map(ToLeanStr::to_lean_str),
                        _ => None,
                    }
                })
                .collect::<Option<Vec<_>>>(),
            _ => None,
        };
        if names.is_none() {
            self.recipe_error(
                expression,
                "slots must be a literal array of declared slot names",
            );
        }
        names
    }

    fn recipe_compounds(
        &mut self,
        expression: &Expression<'a>,
        axes: &BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        slots: Option<&BTreeSet<LeanStr>>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        preview_scopes: &mut Vec<RecipePreviewScope>,
    ) {
        let Expression::ArrayExpression(compounds) = expression.get_inner_expression() else {
            self.recipe_error(expression, "compoundVariants requires a literal array");
            self.recipe_replace(expression, "[]");
            return;
        };
        for compound in &compounds.elements {
            let Some(compound) = compound.as_expression() else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidRecipeConfig {
                        reason: "compoundVariants cannot contain spreads or empty slots".into(),
                    },
                    self.source_span(compound.span()),
                ));
                self.omit_input(compound.span());
                self.recipe_replacements.push(RecipeReplacement {
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
            let role_start = self.recipe_roles.len();
            let scope_start = preview_scopes.len();
            let edit_start = self.recipe_replacements.len();
            let mut valid = true;
            let mut compound_conditions = Vec::new();
            if let Some(conditions) = self.recipe_properties(compound) {
                for (axis, condition) in conditions {
                    if matches!(axis.as_str(), "class" | "className") {
                        self.recipe_slot_value(
                            &condition.value,
                            slots,
                            output,
                            values,
                            &[],
                            preview_scopes,
                        );
                    } else if let Some(keys) =
                        self.recipe_choice(&axis, &condition.value, axes, true)
                    {
                        if !keys.is_empty() {
                            compound_conditions.push(RecipeCondition { axis, values: keys });
                        }
                    } else {
                        valid = false;
                    }
                }
                for role in &mut self.recipe_roles[role_start..] {
                    role.conditions.clone_from(&compound_conditions);
                }
            } else {
                valid = false;
            }
            valid &= !self.lint_findings[finding_start..].iter().any(|finding| {
                matches!(
                    finding.diagnostic,
                    LintDiagnostic::InvalidRecipeConfig { .. }
                )
            });
            if valid {
                let compound_values: Vec<_> =
                    values[value_start..].iter().map(|v| v.span).collect();
                if !compound_values.is_empty() {
                    preview_scopes.push(RecipePreviewScope {
                        span: self.source_span(compound.span()),
                        values: compound_values,
                    });
                }
            } else {
                values.truncate(value_start);
                self.recipe_roles.truncate(role_start);
                preview_scopes.truncate(scope_start);
                self.recipe_replacements.truncate(edit_start);
                self.recipe_replace(compound, "...[]");
            }
        }
    }

    fn recipe_properties<'e>(
        &mut self,
        expression: &'e Expression<'a>,
    ) -> Option<Vec<(LeanStr, &'e ObjectProperty<'a>)>> {
        let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
            self.recipe_error(expression, "expected a literal object");
            self.recipe_replace(expression, "{}");
            return None;
        };
        Some(self.recipe_object_properties(object))
    }

    fn recipe_object_properties<'e>(
        &mut self,
        object: &'e ObjectExpression<'a>,
    ) -> Vec<(LeanStr, &'e ObjectProperty<'a>)> {
        let mut properties = Vec::new();
        let mut keys = BTreeSet::new();
        for entry in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = entry else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidRecipeConfig {
                        reason: "objects cannot contain spreads".into(),
                    },
                    self.source_span(entry.span()),
                ));
                self.omit_input(entry.span());
                self.recipe_replacements.push(RecipeReplacement {
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
                self.recipe_error(&property.value, "keys must be static");
                self.recipe_omit_property(property);
                continue;
            };
            if property.method
                || property.kind != PropertyKind::Init
                || property.computed
                || name.value == "__proto__"
                || !keys.insert(name.value.clone())
            {
                self.recipe_error(&property.value, "expected unique plain static properties");
                self.recipe_omit_property(property);
                continue;
            }
            properties.push((name.value, property.as_ref()));
        }
        properties
    }

    fn recipe_choice(
        &mut self,
        axis: &str,
        expression: &Expression<'a>,
        variants: &BTreeMap<LeanStr, BTreeSet<LeanStr>>,
        multiple: bool,
    ) -> Option<Vec<LeanStr>> {
        let Some(options) = variants.get(axis) else {
            self.recipe_error(expression, "condition names an unknown variant axis");
            return None;
        };
        if multiple && let Expression::ArrayExpression(array) = expression.get_inner_expression() {
            let mut valid = true;
            let mut keys = Vec::new();
            for element in &array.elements {
                if let Some(value) = element.as_expression() {
                    if let Some(nested) = self.recipe_choice(axis, value, variants, false) {
                        keys.extend(nested);
                    } else {
                        valid = false;
                    }
                } else {
                    self.recipe_error(
                        expression,
                        "conditions cannot contain spreads or empty slots",
                    );
                    valid = false;
                }
            }
            return valid.then_some(keys);
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
                self.recipe_error(expression, "conditions must be literal option values");
                return None;
            }
        };
        if key.as_ref().is_some_and(|key| !options.contains(key)) {
            self.recipe_error(expression, "condition names an unknown variant option");
            return None;
        }
        Some(key.into_iter().collect())
    }

    /// Extracts one value and records its role: the `slots` that it styles, and
    /// whether it is an `overlay` on their base values. An invalid value is reported
    /// and becomes an empty value.
    fn recipe_value(
        &mut self,
        expression: &Expression<'a>,
        output: OutputFormat,
        values: &mut Vec<StyleCall>,
        slots: &[LeanStr],
        overlay: bool,
        conditions: &[RecipeCondition],
    ) {
        self.recipe_roles.push(RecipeValueRole {
            span: self.source_span(expression.span()),
            slots: slots.to_vec(),
            overlay,
            conditions: conditions.to_vec(),
        });
        if !self.valid_recipe_value(expression) {
            self.omit_input(expression.span());
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidRecipeInput,
                self.source_span(expression.span()),
            ));
            values.push(StyleCall {
                span: self.source_span(expression.span()),
                key: self.style_key(self.source_span(expression.span())),
                output,
                inputs: Vec::new(),
            });
            return;
        }
        let mut inputs = Vec::new();
        self.in_recipe = true;
        self.collect_style_expression(expression, &mut inputs);
        self.in_recipe = false;
        values.push(StyleCall {
            span: self.source_span(expression.span()),
            key: self.style_key(self.source_span(expression.span())),
            output,
            inputs,
        });
    }

    fn valid_recipe_value(&mut self, expression: &Expression<'a>) -> bool {
        match expression.get_inner_expression() {
            Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            Expression::Identifier(_) => is_undefined(expression),
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
                element
                    .as_expression()
                    .is_some_and(|value| self.valid_recipe_value(value))
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
                        && self.valid_recipe_value(&property.value))
            }),
            _ => false,
        }
    }

    fn recipe_replace(&mut self, expression: &Expression<'a>, replacement: &str) {
        self.omit_input(expression.span());
        self.recipe_replacements.push(RecipeReplacement {
            span: self.source_span(expression.span()),
            replacement: replacement.into(),
        });
    }

    fn recipe_omit_property(&mut self, property: &ObjectProperty<'a>) {
        self.omit_input(property.span);
        self.recipe_replacements.push(RecipeReplacement {
            span: self.source_span(property.span),
            replacement: "...{}".into(),
        });
    }

    fn recipe_error(&mut self, expression: &Expression<'a>, reason: &str) {
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidRecipeConfig {
                reason: reason.into(),
            },
            self.source_span(expression.span()),
        ));
    }
}

/// Whether `name` can name a slot without being read as a utility input key or as
/// a class prop.
fn is_slot_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with("--")
        && !name.ends_with(':')
        && !matches!(name, "class" | "className")
}

fn is_undefined(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::Identifier(value) if value.name == "undefined")
}
