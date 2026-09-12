//! `style.animation()` declarations and their uses.
//!
//! A declaration is one object of static keyframe utility strings keyed by `from`,
//! `to`, or percentage steps. A use is a call on the declared binding with exactly one
//! literal options string. Both are validated here so the declaration-site identity and
//! its frames are known before the compiler runs.

use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKind};
use oxc::span::GetSpan;

use stillwind_ir::{AnimationDefinition, AnimationFrame, AssetReference, ClassExpression};
use stillwind_semantic::{AssetKind, ExportedValue};
use stillwind_source::{CompileError, Span, Spanned};

use crate::state::MacroExtractor;

/// A resolved animation use, split by whether the animation is already known.
///
/// The static case keeps the single asset; the dynamic case keeps every candidate the
/// frontend proved plus the span of the whole expression, so the runtime can select
/// among prebuilt declarations.
pub(crate) enum ParsedAnimationUse {
    /// Exactly one animation identity.
    Static {
        /// The declared animation.
        animation: AssetReference,
        /// The options string, with the span of its contents.
        options: Spanned<String>,
    },
    /// Several possible animations; the runtime picks one.
    Dynamic {
        /// Span of the whole use expression, for the runtime descriptor.
        value: Span,
        /// Candidate animations, each a declared animation.
        animations: Vec<AssetReference>,
        /// The options string, shared by every candidate.
        options: Spanned<String>,
    },
}

impl ParsedAnimationUse {
    /// Converts the use into the IR class expression that mirrors its shape.
    pub(crate) fn into_class_expression(self) -> ClassExpression {
        match self {
            Self::Static { animation, options } => {
                ClassExpression::AnimationUse { animation, options }
            }
            Self::Dynamic {
                value,
                animations,
                options,
            } => ClassExpression::DynamicAnimationUse {
                value,
                animations,
                options,
            },
        }
    }
}

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to the animations it may denote.
    ///
    /// Checker facts win; otherwise the expression must be a local binding of a
    /// declared animation. `None` means it is not an animation value, so callers should
    /// fall through to the other expression forms.
    pub(crate) fn animation_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Animations(value)) = self.type_facts.get(&(span.start, span.end))
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
        let animation = self.animation_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            identity: self.identity(AssetKind::Animation, animation),
            span: self.source_span(span),
            local_declaration: Some(animation.span),
        }])
    }

    /// Recognizes `<animation>(options)` and validates the options argument.
    ///
    /// A use with zero candidates resolves to `None` rather than to an empty dynamic
    /// use; a wrong arity or a non-literal options value is reported and treated as
    /// unusable.
    pub(crate) fn animation_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<ParsedAnimationUse> {
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
            [animation] => Some(ParsedAnimationUse::Static {
                animation: animation.clone(),
                options,
            }),
            [] => None,
            _ => Some(ParsedAnimationUse::Dynamic {
                value: self.source_span(call.span),
                animations,
                options,
            }),
        }
    }

    /// Validates and records one `style.animation({...})` declaration.
    ///
    /// Every frame must be a plain property with a static key and a literal utility
    /// string; invalid frames are reported and skipped, and a declaration with no valid
    /// frame is rejected so no identity is created for it.
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

/// True for the keyframe selectors Stillwind accepts.
///
/// `from`, `to`, or a comma-separated list of unsigned integer percentages; a signed or
/// fractional step is rejected because the generated keyframe rule would not match what
/// was written.
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
