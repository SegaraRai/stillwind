//! Lowering of the expressions a `style()` call accepts.
//!
//! The mapping is structure-preserving: a conditional stays a conditional, an array
//! stays an array, a variant wrapper stays a wrapper, and each authored expression is
//! lowered exactly once. That is what lets the compiler emit one JavaScript expression
//! per user expression with short-circuiting intact.
//!
//! Anything the frontend cannot resolve locally is not guessed: the expression is
//! recorded as a type probe (when a checker could answer) or reported as a diagnostic
//! (when it cannot be a class value at all). Every produced span is a span of the
//! analysed source.

use oxc::ast::ast::{
    ArrayExpression, ArrayExpressionElement, CallExpression, Expression, ObjectExpression,
    ObjectProperty, ObjectPropertyKind, PropertyKind,
};
use oxc::span::GetSpan;
use oxc::syntax::number::ToJsString;
use oxc::syntax::operator::{LogicalOperator, UnaryOperator};

use stillwind_diagnostics::{CompileError, CompileErrorKind, VariantInputErrorKind};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalBranches, CustomPropertyEntry, GroupEntry,
    RuleExpression, RuleExpressionError, RuleExpressionErrorKind, StyleInput, StyleVariant,
};
use stillwind_semantic::{ExportedValue, IdentityId};
use stillwind_source::Span;

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
    /// order. `None` means the value is not a resolvable style candidate at all.
    pub(crate) fn style_candidates(&self, expression: &Expression<'a>) -> Option<Vec<IdentityId>> {
        self.record_type_probe(expression);
        let span = expression.span();
        if let Some(ExportedValue::Styles(value)) = self.type_fact(span) {
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
                let consequent = self.style_candidates(&conditional.consequent);
                let alternate = self.style_candidates(&conditional.alternate);
                let mut candidates = consequent?;
                candidates.extend(alternate?);
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

    /// Records an expression the host checker should resolve.
    ///
    /// Literals are skipped because their type is already known, and the probe set
    /// deduplicates spans. Probes are what keep a module of literal utilities from
    /// asking the checker anything.
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
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::NestedStyleCall,
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
        if let Some(candidates) = self.style_candidates(expression) {
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
                        self.diagnostics.push(CompileError::new(
                            CompileErrorKind::StyleArraySpread,
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

    /// Lowers one class-valued expression while mirroring the source structure.
    ///
    /// Returns `None` when the expression contributes no class, such as `null`,
    /// `undefined`, or `false`. Conditional tests remain even when both branches are
    /// empty.
    ///
    /// Literal array spreads are expanded; empty slots contribute no class.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    fn class_expression(&mut self, expression: &Expression<'a>) -> Option<ClassExpression> {
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
                                self.diagnostics.push(CompileError::new(
                                    CompileErrorKind::StyleArraySpread,
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
            Expression::BooleanLiteral(value) if !value.value => None,
            Expression::Identifier(value) if value.name == "undefined" => None,
            Expression::Identifier(_) => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::RefFactsRequired,
                    self.source_span(expression.span()),
                ));
                None
            }
            _ => {
                if let Some(StyleInput::Class(class)) = self.collect_ref_projection(expression) {
                    return Some(class);
                }
                if let Some(keyframes) = self.keyframes_use(expression) {
                    return Some(keyframes.into_class_expression());
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
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidStyleInput,
                    self.source_span(expression.span()),
                ));
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
    /// properties, and a key ending in `:` becomes a variant
    /// wrapper. Computed keys resolve to literal names or ref variants. Literal
    /// object spreads are expanded with the last value of each known key.
    fn object_class_expression(
        &mut self,
        object: &ObjectExpression<'a>,
    ) -> Option<ClassExpression> {
        let mut entries = Vec::new();
        let mut properties = Vec::new();
        self.collect_object_properties(object, &mut properties);
        for property in properties {
            if property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidAttributeProperty,
                    self.source_span(property.span),
                ));
                continue;
            }
            let name = self.static_property_name(&property.key);
            if property.computed && name.is_none() {
                let Some(variant) = property
                    .key
                    .as_expression()
                    .and_then(|key| self.ref_variant(key))
                else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidComputedAttribute,
                        self.source_span(property.key.span()),
                    ));
                    continue;
                };
                if let Some(entry) = self.variant_entry(variant, &property.value) {
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
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidAttributeKey,
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
            if !name.value.ends_with(':') || name.value == ":" {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidAttributeKey,
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

    /// Expands literal argument spreads without changing ordinary argument lowering.
    pub(crate) fn collect_style_spread(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        let Expression::ArrayExpression(array) = expression.get_inner_expression() else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::StyleArgumentSpread,
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
                        self.diagnostics.push(CompileError::new(
                            CompileErrorKind::AttributeSpread,
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

    /// Returns one variant-wrapped rule entry when every contributing descendant can
    /// produce CSS. Returns `None` for an empty value or after recording the first
    /// incompatible descendant.
    fn variant_entry(
        &mut self,
        variant: StyleVariant,
        value: &Expression<'a>,
    ) -> Option<GroupEntry> {
        let mut inputs = Vec::new();
        self.collect_style_expression(value, &mut inputs);
        if inputs.is_empty() {
            return None;
        }
        let inputs = match inputs
            .iter()
            .map(rule_class_input)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(inputs) => inputs,
            Err(failure) => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidVariantInput {
                        reason: failure.reason,
                    },
                    failure.span,
                ));
                return None;
            }
        };
        Some(GroupEntry::Class(Box::new(
            RuleExpression::Variant { variant, inputs }.into(),
        )))
    }
}

/// A failed variant input conversion with the first incompatible input's kind and span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VariantInputError {
    reason: VariantInputErrorKind,
    span: Span,
}

impl From<RuleExpressionError> for VariantInputError {
    fn from(failure: RuleExpressionError) -> Self {
        let reason = match failure.reason {
            RuleExpressionErrorKind::Attribute => VariantInputErrorKind::Attribute,
            RuleExpressionErrorKind::CustomProperty => VariantInputErrorKind::CustomProperty,
            RuleExpressionErrorKind::ExternalClass => VariantInputErrorKind::ExternalClass,
        };
        Self {
            reason,
            span: failure.span,
        }
    }
}

/// Returns a rule subtree for a root input when every nested leaf supports CSS
/// variants. An incompatible descendant carries its own source range.
fn rule_class_input(input: &StyleInput) -> Result<RuleExpression, VariantInputError> {
    match input {
        StyleInput::Class(class) => RuleExpression::try_from(class.clone()).map_err(Into::into),
        StyleInput::Array(inputs) => Ok(RuleExpression::Array(
            inputs
                .iter()
                .map(rule_class_input)
                .collect::<Result<_, _>>()?,
        )),
        StyleInput::Conditional { test, branches } => Ok(RuleExpression::Conditional {
            test: *test,
            branches: branches.as_ref().try_map(|values| {
                values
                    .iter()
                    .map(rule_class_input)
                    .collect::<Result<_, _>>()
            })?,
        }),
        StyleInput::ViewTransitionUse { transition, name } => {
            Ok(RuleExpression::ViewTransitionUse {
                transition: transition.clone(),
                name: name.clone(),
            })
        }
        StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        } => Ok(RuleExpression::DynamicViewTransitionUse {
            value: *value,
            transitions: transitions.clone(),
            name: name.clone(),
        }),
        StyleInput::Composition { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::Composition,
            span: *value,
        }),
        StyleInput::RefAttachment { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::RefAttachment,
            span: *value,
        }),
        StyleInput::DynamicRefAttachment { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::DynamicRefAttachment,
            span: *value,
        }),
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
