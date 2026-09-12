//! `style.viewTransition()` declarations and their uses.
//!
//! A declaration is an object with any of `group`, `old`, `new`, each a literal utility
//! string; the generated `view-transition-name` and its pseudo-element rules come from
//! that declaration identity. A use may add a stable key through `.for(key)`, which
//! stays a user expression rather than being evaluated here.
//!
//! Keyed uses are always dynamic in the IR even when only one transition is possible,
//! because the key expression must be evaluated at runtime for every instance that
//! renders.

use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKind};
use oxc::span::GetSpan;

use stillwind_ir::{AssetReference, StyleInput, ViewTransitionDefinition};
use stillwind_semantic::{AssetKind, ExportedValue};
use stillwind_source::{CompileError, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to the view transitions it may denote.
    ///
    /// Facts win over local bindings, exactly as for animations; `None` means the
    /// expression is not a view-transition value.
    pub(crate) fn view_transition_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::ViewTransitions(value)) =
            self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity, span)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let transition = self.view_transition_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            identity: self.identity(AssetKind::ViewTransition, transition),
            span: self.source_span(span),
            local_declaration: Some(transition.span),
        }])
    }

    /// Recognizes `<transition>` and `<transition>.for(key)` uses.
    ///
    /// A `.for(...)` argument is kept as a user span, never evaluated. A use that the
    /// checker already described through a type fact becomes dynamic even when only one
    /// candidate remains. A singleton fact does not prove runtime variation; this is
    /// the lowering policy for fact-backed uses.
    pub(crate) fn view_transition_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let expression_has_type_fact = self
            .type_facts
            .contains_key(&(expression.span().start, expression.span().end));
        let (value, transitions, key) = match expression {
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                if member.property.name.as_str() != "for" {
                    return None;
                }
                let transitions = self.view_transition_references(&member.object)?;
                if call.arguments.len() != 1 {
                    self.diagnostics.push(CompileError::new(
                        "SHVIEW002",
                        "viewTransition.for() requires one stable key expression",
                        self.source_span(call.span),
                    ));
                    return None;
                }
                (
                    self.source_span(member.object.span()),
                    transitions,
                    Some(self.source_span(call.arguments[0].span())),
                )
            }
            _ => (
                self.source_span(expression.span()),
                self.view_transition_references(expression)?,
                None,
            ),
        };
        if expression_has_type_fact && key.is_none() {
            return (!transitions.is_empty()).then_some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                key: None,
            });
        }
        match transitions.as_slice() {
            [transition] => Some(StyleInput::ViewTransitionUse {
                transition: transition.clone(),
                key,
            }),
            [] => None,
            _ => Some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                key,
            }),
        }
    }

    /// Validates and records one `style.viewTransition({...})` declaration.
    ///
    /// Only `group`, `old` and `new` are accepted, each with a literal utility string.
    /// Unknown or non-literal keys are reported and skipped; the result needs at least
    /// one of the three to be usable.
    pub(crate) fn collect_view_transition_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ViewTransitionDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHVIEW001",
                "style.viewTransition() requires one object of static utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(Expression::ObjectExpression(object)) = call.arguments[0].as_expression() else {
            self.diagnostics.push(CompileError::new(
                "SHVIEW001",
                "style.viewTransition() requires one object of static utility strings",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut group = None;
        let mut old = None;
        let mut new = None;
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition definitions cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition definitions must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(property_name) = self.property_name(&property.key) else {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let Expression::StringLiteral(value) = &property.value else {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition values must be literal utility strings",
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            let value = self.string_literal(value);
            match property_name.value.as_str() {
                "group" => group = Some(value),
                "old" => old = Some(value),
                "new" => new = Some(value),
                _ => self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    property_name.span,
                )),
            }
        }
        if group.is_none() && old.is_none() && new.is_none() {
            self.diagnostics.push(CompileError::new(
                "SHVIEW001",
                "style.viewTransition() requires at least one of group, old, or new",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(ViewTransitionDefinition {
            name,
            call_span: self.source_span(call.span),
            group,
            old,
            new,
        })
    }
}
