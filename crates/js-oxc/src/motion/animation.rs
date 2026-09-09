use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKind};
use oxc::span::GetSpan;

use stillwind_ir::{AnimationDefinition, AnimationFrame, AssetReference, StyleInput};
use stillwind_semantic::ExportedValue;
use stillwind_source::{CompileError, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn animation_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.type_probes.borrow_mut().insert((span.start, span.end));
        if let Some(ExportedValue::Animations(value)) = self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let animation = self.animation_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            module_path: self.source.path().to_owned(),
            name: animation.clone(),
        }])
    }

    pub(crate) fn animation_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
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
        let Some(Expression::StringLiteral(options)) = call.arguments[0].as_expression() else {
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

    pub(crate) fn collect_animation_definition(
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
        let Some(Expression::ObjectExpression(object)) = call.arguments[0].as_expression() else {
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
}

fn is_animation_frame(value: &str) -> bool {
    value == "from"
        || value == "to"
        || value.split(',').all(|part| {
            part.trim()
                .strip_suffix('%')
                .is_some_and(|number| number.trim().parse::<u8>().is_ok())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_require_keywords_or_comma_separated_unsigned_percentages() {
        for frame in ["from", "to", "0%", "100%", "0%, 50%, 100%"] {
            assert!(is_animation_frame(frame));
        }
        for frame in ["", "middle", "-1%", "1.5%", "50%,", "256%"] {
            assert!(!is_animation_frame(frame));
        }
    }
}
