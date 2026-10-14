//! `style.keyframes()` declarations and their uses.
//!
//! A declaration is one object with an optional literal `label` beside static keyframe
//! utility strings keyed by `from`, `to`, or percentage steps. A use is a call on the
//! declared binding with exactly one literal options string. Both are validated here so
//! the declaration-site identity and its frames are known before the compiler runs.

use oxc::ast::ast::{
    CallExpression, Expression, ObjectExpression, ObjectPropertyKind, PropertyKind,
};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{
    AssetReference, ClassExpression, KeyframesBody, KeyframesBranch, KeyframesDefinition,
    KeyframesFrame, RuleExpression, StyleVariant,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;

/// A resolved keyframes use, split by whether the keyframes is already known.
///
/// The static case keeps the single asset; the dynamic case keeps every candidate the
/// frontend proved plus the span of the value expression, so the generated lookup reads
/// the name the value carries and selects one prebuilt declaration.
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
        /// The value expression the call is made on, which carries the keyframes name.
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
                RuleExpression::KeyframesUse { keyframes, options }.into()
            }
            Self::Dynamic {
                value,
                keyframes,
                options,
            } => RuleExpression::DynamicKeyframesUse {
                value,
                keyframes,
                options,
            }
            .into(),
        }
    }
}

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to the keyframes it may denote.
    ///
    /// Checker facts win over local declaration values. Failure distinguishes a
    /// confirmed unsupported value from missing information about identities.
    pub(crate) fn keyframes_references(
        &self,
        expression: &Expression<'a>,
    ) -> Result<Vec<AssetReference>, ResolutionFailure> {
        let span = expression.get_inner_expression().span();
        self.asset_candidates(expression, AssetKind::Keyframes)
            .map(|identities| {
                identities
                    .iter()
                    .map(|identity| self.asset_reference(identity, span))
                    .collect()
            })
    }

    /// Recognizes `<keyframes>(options)` and validates the options argument.
    ///
    /// A use with zero candidates resolves to `None` rather than to an empty dynamic
    /// use; a wrong arity or a non-literal options value is reported and treated as
    /// unusable. A recognized call is claimed here, so the unclaimed-use check reports
    /// only calls this lowering never consumed.
    pub(crate) fn keyframes_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<ParsedKeyframesUse> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let keyframes = match self.keyframes_references(&call.callee) {
            Ok(value) => value,
            Err(ResolutionFailure::Unsupported) => return None,
            Err(ResolutionFailure::MissingFacts) => {
                self.consumed_use_spans.insert(LocalSpan::from(call.span));
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::RefFactsRequired,
                    self.source_span(call.callee.span()),
                ));
                return None;
            }
        };
        // Claim the call before validating it, so a rejected use is reported once.
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        if call.arguments.len() != 1 {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidKeyframesOptions,
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(Expression::StringLiteral(options)) = call.arguments[0].as_expression() else {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidKeyframesOptions,
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
                value: self.source_span(call.callee.span()),
                keyframes,
                options,
            }),
        }
    }

    /// Validates and records one `style.keyframes({...})` declaration.
    ///
    /// Every frame must be a plain property with a static key and a literal utility
    /// string, and the declaration may state one literal `label` beside them; invalid
    /// entries are reported and skipped. An empty definition retains its binding
    /// and identity.
    pub(crate) fn collect_keyframes_definition(
        &mut self,
        binding: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> KeyframesDefinition {
        self.omit_input(call.span);
        let object = if call.arguments.len() == 1 {
            call.arguments[0]
                .as_expression()
                .and_then(|expression| match expression {
                    Expression::ObjectExpression(object) => Some(object),
                    _ => None,
                })
        } else {
            None
        };
        if object.is_none() {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidKeyframesDefinition,
                self.source_span(call.span),
            ));
        }
        let mut label = None;
        let body = object.map_or(
            KeyframesBody {
                frames: Vec::new(),
                branches: Vec::new(),
            },
            |object| self.keyframes_body(object, Some(&mut label)),
        );
        KeyframesDefinition {
            binding,
            call_span: self.source_span(call.span),
            label,
            body,
        }
    }

    /// Reads one frames object.
    ///
    /// `label` receives the declaration's own label, so it is `Some` for the
    /// declaration's body only: a conditional replacement frames one condition and
    /// cannot name the declaration.
    fn keyframes_body(
        &mut self,
        object: &ObjectExpression<'a>,
        mut label: Option<&mut Option<Spanned<String>>>,
    ) -> KeyframesBody {
        let mut branches = Vec::new();
        let mut frames = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::KeyframesSpread,
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.method || property.kind != PropertyKind::Init {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidKeyframesProperty,
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
                    let body = self.keyframes_body(nested, None);
                    branches.push(KeyframesBranch { variant, body });
                } else {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::InvalidKeyframesDefinition,
                        self.source_span(property.value.span()),
                    ));
                }
                continue;
            }
            let Some(selector) = key else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidKeyframesFrameKey,
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if selector.value == "label" {
                match label.as_deref_mut() {
                    Some(label) => match &property.value {
                        Expression::StringLiteral(value) => {
                            *label = Some(self.string_literal(value));
                        }
                        _ => self.lint_findings.push(LintFinding::new(
                            LintDiagnostic::InvalidKeyframesLabel,
                            self.source_span(property.value.span()),
                        )),
                    },
                    None => self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::InvalidKeyframesLabel,
                        self.source_span(property.key.span()),
                    )),
                }
                continue;
            }
            if !is_keyframes_frame(&selector.value) {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidKeyframesFrameSelector,
                    selector.span,
                ));
                continue;
            }
            let Expression::StringLiteral(utilities) = &property.value else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidKeyframesFrameValue,
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            frames.push(KeyframesFrame {
                selector,
                utilities: self.string_literal(utilities),
            });
        }
        KeyframesBody { frames, branches }
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
