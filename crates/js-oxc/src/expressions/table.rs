//! Lowers lookups in array and object literals whose values are utility literals.
//!
//! In `style({ file: "icon-a", folder: "icon-b" }[kind])`, the object is a table, and
//! the key selects one entry at runtime. Each entry becomes one branch of a
//! [`ConditionalBranches::Select`], so every entry gets its own conditional class unit.
//! Object keys are lookup keys; they are never read as utilities or variants.
//!
//! Tables accept only literal values so that the type of the lookup matches the lowered
//! values. A table whose entries cannot be determined is omitted as a whole, and an
//! entry with an unsupported value is omitted alone.

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    ArrayExpression, ArrayExpressionElement, Expression, ObjectExpression, ObjectPropertyKind,
    PropertyKey, PropertyKind,
};
use oxc::span::{GetSpan, Span as OxcSpan};
use oxc::syntax::number::ToJsString;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{ConditionalBranches, RuleExpression, SelectEntry};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Lowers `expression` when it reads a property of an array or object literal.
    ///
    /// Returns `None` when `expression` is not such a lookup, and the caller lowers it
    /// as another input. Otherwise returns the rules that the lookup contributes. They
    /// are empty for an omitted table or a literal key that names no entry with a
    /// value. A literal key selects the rules of its entry statically. Any other key
    /// stays one runtime lookup with one branch for each entry that has a value.
    pub(super) fn table_lookup(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<Vec<RuleExpression>> {
        let (table, key) = match expression.get_inner_expression() {
            Expression::ComputedMemberExpression(member) => {
                (&member.object, TableKey::Expression(&member.expression))
            }
            Expression::StaticMemberExpression(member) => (
                &member.object,
                TableKey::Static(member.property.name.to_lean_str()),
            ),
            _ => return None,
        };
        let entries = match table.get_inner_expression() {
            Expression::ArrayExpression(array) => self.array_entries(array),
            Expression::ObjectExpression(object) => self.object_entries(object),
            _ => return None,
        };
        let Some(entries) = entries else {
            self.omit_input(expression.span());
            return Some(Vec::new());
        };
        let literal = match &key {
            TableKey::Static(key) => Some(key.clone()),
            TableKey::Expression(key) => literal_key(key),
        };
        if let Some(literal) = literal {
            return Some(
                entries
                    .into_iter()
                    .find(|entry| entry.key == literal)
                    .map(|entry| entry.value)
                    .unwrap_or_default(),
            );
        }
        let TableKey::Expression(key) = key else {
            return Some(Vec::new());
        };
        if entries.is_empty() {
            return Some(Vec::new());
        }
        Some(vec![RuleExpression::Conditional {
            test: self.source_span(key.span()),
            branches: ConditionalBranches::Select { entries },
        }])
    }

    /// Returns the entries of an array table with a value, keyed by decimal index.
    ///
    /// Literal array spreads are expanded, and holes take an index without an entry.
    /// Returns `None` after reporting a spread of any other value.
    fn array_entries(
        &mut self,
        array: &ArrayExpression<'a>,
    ) -> Option<Vec<SelectEntry<Vec<RuleExpression>>>> {
        let mut values = Vec::new();
        if !self.collect_array_values(array, &mut values) {
            return None;
        }
        Some(
            values
                .into_iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    let value = self.table_value(value?)?;
                    Some(SelectEntry {
                        key: index.to_lean_str(),
                        value,
                    })
                })
                .collect(),
        )
    }

    /// Appends the elements of `array` in index order, with `None` for a hole. Returns
    /// false after reporting a spread that is not an array literal.
    fn collect_array_values<'b>(
        &mut self,
        array: &'b ArrayExpression<'a>,
        values: &mut Vec<Option<&'b Expression<'a>>>,
    ) -> bool {
        for element in &array.elements {
            match element {
                ArrayExpressionElement::SpreadElement(spread) => {
                    let Expression::ArrayExpression(inner) = spread.argument.get_inner_expression()
                    else {
                        self.invalid_table(spread.span);
                        return false;
                    };
                    if !self.collect_array_values(inner, values) {
                        return false;
                    }
                }
                ArrayExpressionElement::Elision(_) => {
                    values.push(None);
                }
                _ => {
                    values.push(element.as_expression());
                }
            }
        }
        true
    }

    /// Returns the entries of an object table with a value, in the position of the
    /// first occurrence of each key and with the value of its last occurrence.
    ///
    /// Literal object spreads are expanded. An accessor or method is reported and keeps
    /// no entry, but it still replaces an earlier entry of the same key. Returns `None`
    /// after reporting a spread of any other value, a key that is not static, or the
    /// key `__proto__`.
    fn object_entries(
        &mut self,
        object: &ObjectExpression<'a>,
    ) -> Option<Vec<SelectEntry<Vec<RuleExpression>>>> {
        let mut properties = Vec::new();
        if !self.collect_object_values(object, &mut properties) {
            return None;
        }
        Some(
            properties
                .into_iter()
                .filter_map(|(key, value)| {
                    Some(SelectEntry {
                        key,
                        value: self.table_value(value?)?,
                    })
                })
                .collect(),
        )
    }

    /// Appends the keys of `object` with their values, replacing the value of a
    /// repeated key in place. An accessor or method has no value. Returns false after
    /// reporting a property that prevents static keys.
    fn collect_object_values<'b>(
        &mut self,
        object: &'b ObjectExpression<'a>,
        properties: &mut Vec<(LeanStr, Option<&'b Expression<'a>>)>,
    ) -> bool {
        for property in &object.properties {
            let (key, value) = match property {
                ObjectPropertyKind::SpreadProperty(spread) => {
                    let Expression::ObjectExpression(inner) =
                        spread.argument.get_inner_expression()
                    else {
                        self.invalid_table(spread.span);
                        return false;
                    };
                    if !self.collect_object_values(inner, properties) {
                        return false;
                    }
                    continue;
                }
                ObjectPropertyKind::ObjectProperty(property) => {
                    let Some(key) = self.table_key(&property.key) else {
                        self.invalid_table(property.key.span());
                        return false;
                    };
                    if key == "__proto__" {
                        self.invalid_table(property.key.span());
                        return false;
                    }
                    let value = if property.method || property.kind != PropertyKind::Init {
                        self.invalid_entry(property.span);
                        None
                    } else {
                        Some(&property.value)
                    };
                    (key, value)
                }
            };
            if let Some(previous) = properties.iter_mut().find(|(other, _)| *other == key) {
                previous.1 = value;
            } else {
                properties.push((key, value));
            }
        }
        true
    }

    /// Returns the property name of a static object key: an identifier, a string, a
    /// number, or a computed literal.
    fn table_key(&mut self, key: &PropertyKey<'a>) -> Option<LeanStr> {
        if let PropertyKey::NumericLiteral(number) = key {
            return Some(number.value.to_js_string().to_lean_str());
        }
        if let Some(name) = self.static_property_name(key) {
            return Some(name.value);
        }
        literal_key(key.as_expression()?)
    }

    /// Lowers one table value into the rules that its entry selects.
    ///
    /// A string or expression-free template becomes one utility list. `null`,
    /// `undefined`, and `false` select no rules and return `None`, as does any other
    /// value after it is reported and omitted.
    fn table_value(&mut self, value: &Expression<'a>) -> Option<Vec<RuleExpression>> {
        match value.get_inner_expression() {
            Expression::StringLiteral(literal) => Some(vec![RuleExpression::Utilities(
                self.string_literal(literal),
            )]),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let Some(text) = template.quasis.first()?.value.cooked.as_ref() else {
                    self.invalid_entry(value.span());
                    return None;
                };
                let text = self.unicode_string(*text, template.span)?.to_lean_str();
                let span = self.source_span(template.span);
                Some(vec![RuleExpression::Utilities(Spanned::new(
                    text,
                    Span::new(span.file, span.start.0 + 1, span.end.0 - 1),
                ))])
            }
            Expression::NullLiteral(_) => None,
            Expression::BooleanLiteral(literal) if !literal.value => None,
            Expression::Identifier(identifier) if identifier.name == "undefined" => None,
            _ => {
                self.invalid_entry(value.span());
                None
            }
        }
    }

    /// Reports a table property or spread that prevents static entries.
    fn invalid_table(&mut self, span: OxcSpan) {
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidStyleTable,
            self.source_span(span),
        ));
    }

    /// Reports and omits a table entry without a literal value.
    fn invalid_entry(&mut self, span: OxcSpan) {
        self.omit_input(span);
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidStyleTableEntry,
            self.source_span(span),
        ));
    }
}

/// The key of a table lookup: a static member name or a computed expression.
enum TableKey<'e, 'a> {
    /// `table.name`.
    Static(LeanStr),
    /// `table[expression]`.
    Expression(&'e Expression<'a>),
}

/// Returns the property name of a literal key expression: a string, an expression-free
/// template, or a number in JavaScript's string form.
fn literal_key(key: &Expression<'_>) -> Option<LeanStr> {
    match key.get_inner_expression() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()?.to_lean_str()),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => Some(
            template
                .quasis
                .first()?
                .value
                .cooked
                .as_ref()?
                .as_str()?
                .to_lean_str(),
        ),
        Expression::NumericLiteral(number) => Some(number.value.to_js_string().to_lean_str()),
        _ => None,
    }
}
