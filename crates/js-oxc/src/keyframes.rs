//! `style.keyframes()` declarations and their uses.
//!
//! A declaration is one object with an optional literal `label` beside static keyframe
//! utility strings keyed by `from`, `to`, or percentage steps. A use is a call on the
//! declared binding with one literal options string, and invalid options lower as empty
//! options. Both are validated here so the declaration-site identity and its frames are
//! known before the compiler runs.

use lean_string::LeanStr;
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
        options: Spanned<LeanStr>,
    },
    /// Several possible keyframes; the runtime picks one.
    Dynamic {
        /// The value expression the call is made on, which carries the keyframes name.
        value: Span,
        /// Candidate keyframes, each a declared keyframes.
        keyframes: Vec<AssetReference>,
        /// The options string, shared by every candidate.
        options: Spanned<LeanStr>,
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

    /// Recognizes `<keyframes>(options)` and lowers it into a class expression.
    ///
    /// The call is a keyframes use when at least one receiver branch resolves to
    /// keyframes. A wrong arity or a nonliteral options value adds a lint and lowers
    /// the use with empty options. A conditional receiver that
    /// [`needs_branch_lowering`](Self::needs_branch_lowering) lowers one use per
    /// branch, and a branch that is not keyframes adds a lint and contributes nothing.
    /// A recognized call is claimed here, so the unclaimed-use check reports only calls
    /// this lowering never consumed.
    ///
    /// Returns `None` for unrelated calls, for missing checker facts, and for a use
    /// that contributes nothing.
    pub(crate) fn keyframes_use(&mut self, expression: &Expression<'a>) -> Option<ClassExpression> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        match self.keyframes_references(&call.callee) {
            Ok(_) => {}
            Err(ResolutionFailure::Unsupported) => return None,
            Err(ResolutionFailure::MissingFacts) => {
                self.consumed_use_spans.insert(LocalSpan::from(call.span));
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::KeyframesFactsRequired,
                    self.source_span(call.callee.span()),
                ));
                return None;
            }
        }
        // Claim the call before validating it, so a rejected use is reported once.
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        let options = self.keyframes_options(call);
        if !self.needs_branch_lowering(&call.callee, AssetKind::Keyframes) {
            return self.keyframes_receiver_use(&call.callee, &options);
        }
        self.lower_receiver_branches(&call.callee, &mut |this, receiver| {
            this.keyframes_receiver_use(receiver, &options)
        })
    }

    /// Reads the options argument of a keyframes use. A wrong arity or a nonliteral
    /// value adds a lint and returns empty options at the call span.
    fn keyframes_options(&mut self, call: &CallExpression<'a>) -> Spanned<LeanStr> {
        if let [argument] = call.arguments.as_slice()
            && let Some(Expression::StringLiteral(options)) = argument.as_expression()
        {
            return self.string_literal(options);
        }
        let span = match call.arguments.as_slice() {
            [argument] => argument.span(),
            _ => call.span,
        };
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidKeyframesOptions,
            self.source_span(span),
        ));
        Spanned::new(LeanStr::new(), self.source_span(span))
    }

    /// Lowers a keyframes use for `receiver`. A receiver with several candidates
    /// selects its keyframes at runtime.
    fn keyframes_receiver_use(
        &mut self,
        receiver: &Expression<'a>,
        options: &Spanned<LeanStr>,
    ) -> Option<ClassExpression> {
        let keyframes = match self.keyframes_references(receiver) {
            Ok(keyframes) => keyframes,
            Err(failure) => {
                self.report_resolution_failure(
                    receiver,
                    failure,
                    LintDiagnostic::InvalidKeyframesReceiver,
                    CompileDiagnostic::KeyframesFactsRequired,
                );
                return None;
            }
        };
        let options = options.clone();
        let parsed = match keyframes.as_slice() {
            [keyframes] => ParsedKeyframesUse::Static {
                keyframes: keyframes.clone(),
                options,
            },
            [] => return None,
            _ => ParsedKeyframesUse::Dynamic {
                value: self.source_span(receiver.span()),
                keyframes,
                options,
            },
        };
        Some(parsed.into_class_expression())
    }

    /// Validates and records one `style.keyframes({...})` declaration.
    ///
    /// Every frame must be a plain property with a static key and a literal utility
    /// string, and the declaration may state one literal `label` beside them; invalid
    /// entries are reported and skipped. An empty definition retains its binding
    /// and identity.
    pub(crate) fn collect_keyframes_definition(
        &mut self,
        binding: Spanned<LeanStr>,
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
        mut label: Option<&mut Option<Spanned<LeanStr>>>,
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
