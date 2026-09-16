//! `style.viewTransition()` declarations and their uses.
//!
//! A declaration is an object with any of `group`, `old`, `new`, `imagePair`, each a
//! literal utility string; the generated `view-transition-name` and its pseudo-element
//! rules come from that declaration identity. A use may add a stable key through
//! `.for(key)`, which stays a user expression rather than being evaluated here.
//!
//! Keyed uses are always dynamic in the IR even when only one transition is possible,
//! because the key expression must be evaluated at runtime for every instance that
//! renders.

use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKind};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{
    AssetReference, StyleInput, ViewTransitionDefinition, ViewTransitionKey,
    ViewTransitionKeyApplication, ViewTransitionName,
};
use stillwind_semantic::{AssetKind, ExportedValue};
use stillwind_source::Spanned;

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to the view transitions it may denote, and whether the value
    /// already carries the key its participants are paired under.
    ///
    /// Facts win over local bindings, exactly as for animations; `None` means the
    /// expression is not a view-transition value.
    pub(crate) fn view_transition_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<(Vec<AssetReference>, bool)> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(fact) = self.type_facts.get(&(span.start, span.end)) {
            let (value, keyed) = match fact {
                ExportedValue::ViewTransitions(value) => (value, false),
                ExportedValue::ViewTransitionKeys(value) => (value, true),
                _ => return None,
            };
            let references = value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity, span)))
                .collect::<Option<Vec<AssetReference>>>()?;
            return Some((references, keyed));
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(transition) = self.view_transition_symbols.get(&symbol_id) {
            return Some((
                vec![AssetReference {
                    identity: self.identity(AssetKind::ViewTransition, transition),
                    span: self.source_span(span),
                    local_declaration: Some(transition.span),
                }],
                false,
            ));
        }
        // A binding whose `.for(...)` was applied where the transition was declared carries
        // its key, so the use names itself from the value.
        let keyed = self.view_transition_key_symbols.get(&symbol_id)?;
        Some((
            keyed
                .iter()
                .map(|identity| AssetReference {
                    identity: identity.clone(),
                    span: self.source_span(span),
                    local_declaration: None,
                })
                .collect(),
            true,
        ))
    }

    /// Recognizes `<transition>` and `<transition>.for(key)` uses.
    ///
    /// A literal key folds here and any other key expression is kept as a user span, never
    /// evaluated. A use that the checker already described through a type fact becomes
    /// dynamic even when only one candidate remains, and a value that carries a key of its
    /// own is named from the value at runtime.
    pub(crate) fn view_transition_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let expression_has_type_fact = self
            .type_facts
            .contains_key(&(expression.span().start, expression.span().end));
        let (value, transitions, name) = if let Expression::CallExpression(call) = expression {
            let Expression::StaticMemberExpression(member) = &call.callee else {
                return None;
            };
            if member.property.name.as_str() != "for" {
                return None;
            }
            let (transitions, _) = self.view_transition_references(&member.object)?;
            let key = self.view_transition_key(call)?;
            (
                self.source_span(member.object.span()),
                transitions,
                ViewTransitionName::Key(key),
            )
        } else {
            let (transitions, keyed) = self.view_transition_references(expression)?;
            let name = if keyed {
                ViewTransitionName::Value
            } else {
                ViewTransitionName::None
            };
            (self.source_span(expression.span()), transitions, name)
        };
        if expression_has_type_fact && matches!(name, ViewTransitionName::None) {
            return (!transitions.is_empty()).then_some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                name,
                guard: Vec::new(),
            });
        }
        match transitions.as_slice() {
            [transition] => Some(StyleInput::ViewTransitionUse {
                transition: transition.clone(),
                name,
                guard: Vec::new(),
            }),
            [] => None,
            _ => Some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                name,
                guard: Vec::new(),
            }),
        }
    }

    /// Reads the key one `.for(...)` call states.
    /// Collects one `transition.for(key)` application written outside a `style()` call.
    ///
    /// A style-call argument consumes its own `.for`, so only a binding initializer reaches
    /// here; the value it builds carries the key to the uses that follow.
    pub(crate) fn collect_view_transition_key_application(
        &mut self,
        call: &CallExpression<'a>,
    ) -> Option<ViewTransitionKeyApplication> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name.as_str() != "for" {
            return None;
        }
        let (transitions, _) = self.view_transition_references(&member.object)?;
        let key = self.view_transition_key(call)?;
        Some(ViewTransitionKeyApplication {
            call_span: self.source_span(call.span),
            value: self.source_span(member.object.span()),
            key,
            transitions,
        })
    }

    ///
    /// A literal key folds at compile time and any other expression is read by the runtime.
    /// More than one argument is reported and contributes nothing.
    fn view_transition_key(&mut self, call: &CallExpression<'a>) -> Option<ViewTransitionKey> {
        let argument = match call.arguments.len() {
            0 => return Some(ViewTransitionKey::Literal(String::new())),
            1 => call.arguments[0].as_expression()?,
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidViewTransitionKeyExpression,
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        match argument {
            Expression::StringLiteral(value) => {
                Some(ViewTransitionKey::Literal(self.string_literal(value).value))
            }
            Expression::NumericLiteral(value) => {
                let raw = self.source.slice(self.source_span(value.span())).ok()?;
                Some(ViewTransitionKey::Literal(raw.to_owned()))
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let cooked = template.quasis[0].value.cooked.as_ref()?;
                Some(ViewTransitionKey::Literal(cooked.to_string()))
            }
            argument => Some(ViewTransitionKey::Dynamic(
                self.source_span(argument.span()),
            )),
        }
    }

    /// Validates and records one `style.viewTransition({...})` declaration.
    ///
    /// Only `group`, `old`, `new` and `imagePair` are accepted, each with a literal
    /// utility string. Unknown or non-literal keys are reported and skipped; the result
    /// needs at least one of the four to be usable.
    pub(crate) fn collect_view_transition_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ViewTransitionDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionDefinition,
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(Expression::ObjectExpression(object)) = call.arguments[0].as_expression() else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionDefinition,
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut group = None;
        let mut old = None;
        let mut new = None;
        let mut image_pair = None;
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::ViewTransitionSpread,
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidViewTransitionProperty,
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(property_name) = self.property_name(&property.key) else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidViewTransitionKey,
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let Expression::StringLiteral(value) = &property.value else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidViewTransitionValue,
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            let value = self.string_literal(value);
            match property_name.value.as_str() {
                "group" => group = Some(value),
                "old" => old = Some(value),
                "new" => new = Some(value),
                "imagePair" => image_pair = Some(value),
                _ => self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidViewTransitionKey,
                    property_name.span,
                )),
            }
        }
        if group.is_none() && old.is_none() && new.is_none() && image_pair.is_none() {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::EmptyViewTransition,
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
            image_pair,
        })
    }
}
