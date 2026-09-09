use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKey, PropertyKind};
use oxc::span::GetSpan;

use stillwind_ir::{AnimationDefinition, AnimationFrame, StyleInput, ViewTransitionDefinition};
use stillwind_source::{CompileError, Spanned};

use super::labels::is_animation_frame;
use super::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(super) fn view_transition_use(
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

    pub(super) fn animation_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let animations = self.animation_references(&call.callee)?;
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::StringLiteral(options) = call.arguments[0].to_expression() else {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let options = self.string_literal(options);
        match animations.as_slice() {
            [animation] => Some(StyleInput::AnimationUse {
                animation: animation.clone(),
                options,
            }),
            [] => None,
            _ => Some(StyleInput::DynamicAnimationUse {
                value: self.source_span(call.span),
                animations,
                options,
            }),
        }
    }

    pub(super) fn property_name(&self, key: &PropertyKey<'a>) -> Option<Spanned<String>> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            )),
            PropertyKey::StringLiteral(literal) => Some(self.string_literal(literal)),
            _ => None,
        }
    }

    pub(super) fn collect_animation_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<AnimationDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut frames = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation keyframes cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation keyframes must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(selector) = self.property_name(&property.key) else {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or percentage keys",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if !is_animation_frame(&selector.value) {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or comma-separated percentages",
                    selector.span,
                ));
                continue;
            }
            let Expression::StringLiteral(utilities) = &property.value else {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation frame values must be literal utility strings",
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            frames.push(AnimationFrame {
                selector,
                utilities: self.string_literal(utilities),
            });
        }
        if frames.is_empty() {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION001",
                "style.animation() requires at least one valid keyframe",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(AnimationDefinition {
            name,
            call_span: self.source_span(call.span),
            frames,
        })
    }

    pub(super) fn collect_view_transition_definition(
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
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
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
