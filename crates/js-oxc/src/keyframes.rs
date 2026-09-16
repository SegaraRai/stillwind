//! `style.keyframes()` declarations and their uses.
//!
//! A declaration is one object of static keyframe utility strings keyed by `from`,
//! `to`, or percentage steps. A use is a call on the declared binding with exactly one
//! literal options string. Both are validated here so the declaration-site identity and
//! its frames are known before the compiler runs.

use oxc::ast::ast::{
    CallExpression, Expression, ObjectExpression, ObjectPropertyKind, PropertyKind,
};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{
    AssetReference, ClassExpression, KeyframesBody, KeyframesBranch, KeyframesDefinition,
    KeyframesFrame, StyleVariant,
};
use stillwind_semantic::{AssetKind, ExportedValue};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

/// A resolved keyframes use, split by whether the keyframes is already known.
///
/// The static case keeps the single asset; the dynamic case keeps every candidate the
/// frontend proved plus the span of the whole expression, so the runtime can select
/// among prebuilt declarations.
pub(crate) enum ParsedKeyframesUse {
    /// Exactly one keyframes identity.
    Static {
        /// The declared keyframes.
        keyframes: AssetReference,
        /// The options string, with the span of its contents.
        options: Spanned<String>,
    },
    /// Several possible keyframes; the runtime picks one.
    Dynamic {
        /// Span of the whole use expression, for the runtime descriptor.
        value: Span,
        /// Candidate keyframes, each a declared keyframes.
        keyframes: Vec<AssetReference>,
        /// The options string, shared by every candidate.
        options: Spanned<String>,
    },
}

impl ParsedKeyframesUse {
    /// Converts the use into the IR class expression that mirrors its shape.
    pub(crate) fn into_class_expression(self) -> ClassExpression {
        match self {
            Self::Static { keyframes, options } => {
                ClassExpression::KeyframesUse { keyframes, options }
            }
            Self::Dynamic {
                value,
                keyframes,
                options,
            } => ClassExpression::DynamicKeyframesUse {
                value,
                keyframes,
                options,
            },
        }
    }
}

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to the keyframes it may denote.
    ///
    /// Checker facts win; otherwise the expression must be a local binding of a
    /// declared keyframes. `None` means it is not an keyframes value, so callers should
    /// fall through to the other expression forms.
    pub(crate) fn keyframes_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        self.record_type_probe(expression);
        if let Some(ExportedValue::Keyframes(value)) = self.type_facts.get(&(span.start, span.end))
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
        let keyframes = self.keyframes_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            identity: self.identity(AssetKind::Keyframes, keyframes),
            span: self.source_span(span),
            local_declaration: Some(keyframes.span),
        }])
    }

    /// Recognizes `<keyframes>(options)` and validates the options argument.
    ///
    /// A use with zero candidates resolves to `None` rather than to an empty dynamic
    /// use; a wrong arity or a non-literal options value is reported and treated as
    /// unusable.
    pub(crate) fn keyframes_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<ParsedKeyframesUse> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let keyframes = self.keyframes_references(&call.callee)?;
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidKeyframesOptions,
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(Expression::StringLiteral(options)) = call.arguments[0].as_expression() else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidKeyframesOptions,
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let options = self.string_literal(options);
        match keyframes.as_slice() {
            [keyframes] => Some(ParsedKeyframesUse::Static {
                keyframes: keyframes.clone(),
                options,
            }),
            [] => None,
            _ => Some(ParsedKeyframesUse::Dynamic {
                value: self.source_span(call.span),
                keyframes,
                options,
            }),
        }
    }

    /// Validates and records one `style.keyframes({...})` declaration.
    ///
    /// Every frame must be a plain property with a static key and a literal utility
    /// string; invalid frames are reported and skipped, and a declaration with no valid
    /// frame is rejected so no identity is created for it.
    pub(crate) fn collect_keyframes_definition(
        &mut self,
        binding: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<KeyframesDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidKeyframesDefinition,
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(Expression::ObjectExpression(object)) = call.arguments[0].as_expression() else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidKeyframesDefinition,
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let body = self.keyframes_body(object)?;
        Some(KeyframesDefinition {
            binding,
            call_span: self.source_span(call.span),
            body,
        })
    }

    fn keyframes_body(&mut self, object: &ObjectExpression<'a>) -> Option<KeyframesBody> {
        let mut branches = Vec::new();
        let mut frames = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::KeyframesSpread,
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidKeyframesProperty,
                    self.source_span(property.span),
                ));
                continue;
            }
            let key = self.static_property_name(&property.key);
            let variant = if key.as_ref().is_some_and(|key| key.value.ends_with(':')) {
                key.clone().map(StyleVariant::Prefix)
            } else if property.computed && key.is_none() {
                property
                    .key
                    .as_expression()
                    .and_then(|key| self.ref_variant(key))
            } else {
                None
            };
            if let Some(variant) = variant {
                if let Expression::ObjectExpression(nested) = property.value.get_inner_expression()
                {
                    if let Some(body) = self.keyframes_body(nested) {
                        branches.push(KeyframesBranch { variant, body });
                    }
                } else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidKeyframesDefinition,
                        self.source_span(property.value.span()),
                    ));
                }
                continue;
            }
            let Some(selector) = key else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidKeyframesFrameKey,
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if !is_keyframes_frame(&selector.value) {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidKeyframesFrameSelector,
                    selector.span,
                ));
                continue;
            }
            let Expression::StringLiteral(utilities) = &property.value else {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidKeyframesFrameValue,
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            frames.push(KeyframesFrame {
                selector,
                utilities: self.string_literal(utilities),
            });
        }
        if frames.is_empty() && branches.is_empty() {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::EmptyKeyframes,
                self.source_span(object.span),
            ));
            return None;
        }
        Some(KeyframesBody { frames, branches })
    }
}

/// True for the keyframe selectors Stillwind accepts.
///
/// `from`, `to`, or a comma-separated list of unsigned integer percentages; a signed or
/// fractional step is rejected because the generated keyframe rule would not match what
/// was written.
fn is_keyframes_frame(value: &str) -> bool {
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
            assert!(is_keyframes_frame(frame));
        }
        for frame in ["", "middle", "-1%", "1.5%", "50%,", "256%"] {
            assert!(!is_keyframes_frame(frame));
        }
    }
}
