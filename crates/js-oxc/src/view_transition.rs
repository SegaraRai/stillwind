//! `style.viewTransition()` declarations and their uses.
//!
//! A declaration is an object with an optional literal `label` and any of `group`,
//! `old`, `new`, `imagePair`, each a literal utility string; the generated
//! `view-transition-name`, its class token, and its pseudo-element rules come from that
//! declaration identity. A use may add a stable key through `.for(key)`, which stays a
//! user expression rather than being evaluated here.
//!
//! Literal keys fold into CSS; expression keys remain use-site spans for runtime name
//! emission. Projection calls have no independently materialized value.

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{CallExpression, Expression, ObjectPropertyKind, PropertyKind};
use oxc::span::GetSpan;

use stillwind_diagnostics::{
    CheckerFactsSubject, CompileDiagnostic, CompileError, LintDiagnostic, LintFinding,
    SourceFailureContext,
};
use stillwind_ir::{
    AssetReference, RuleExpression, StyleInput, ViewTransitionDefinition, ViewTransitionKey,
    ViewTransitionName,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Resolves an expression to its finite view-transition declaration candidates.
    ///
    /// Facts win over local declaration values. Failure distinguishes a confirmed
    /// unsupported value from missing information about identities.
    pub(crate) fn view_transition_references(
        &self,
        expression: &Expression<'a>,
    ) -> Result<Vec<AssetReference>, ResolutionFailure> {
        let span = expression.get_inner_expression().span();
        self.asset_candidates(expression, AssetKind::ViewTransition)
            .map(|identities| {
                identities
                    .iter()
                    .map(|identity| self.asset_reference(identity, span))
                    .collect()
            })
    }

    /// Recognizes `<transition>` and `<transition>.for(key)` uses.
    ///
    /// A literal key folds here and any other key expression is kept as a user span,
    /// never evaluated. A use that the checker already described through a type fact
    /// becomes dynamic even when only one candidate remains. An empty finite candidate
    /// set produces no use node.
    ///
    /// A `.for(key)` use whose receiver
    /// [`needs_branch_lowering`](Self::needs_branch_lowering) lowers one use per branch
    /// and returns them as a class expression. A branch that is not a transition adds a
    /// lint and contributes nothing.
    pub(crate) fn view_transition_use(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let Expression::CallExpression(call) = expression else {
            let transitions = self.view_transition_references(expression).ok()?;
            let value = self.source_span(expression.span());
            let dynamic = self.type_fact(expression.span()).is_some();
            return transition_use(value, transitions, ViewTransitionName::None, dynamic);
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name.as_str() != "for" {
            return None;
        }
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        if let Err(failure) = self.view_transition_references(&member.object) {
            self.report_resolution_failure(
                &member.object,
                failure,
                LintDiagnostic::InvalidViewTransitionReceiver,
                CompileDiagnostic::CheckerFactsRequired {
                    subject: CheckerFactsSubject::ViewTransitionReceiver,
                },
            );
            return None;
        }
        let key = self.view_transition_key(call)?;
        if !self.needs_branch_lowering(&member.object, AssetKind::ViewTransition) {
            return self.view_transition_receiver_use(&member.object, &key);
        }
        self.lower_receiver_branches(&member.object, &mut |this, receiver| match this
            .view_transition_receiver_use(receiver, &key)?
        {
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
        })
        .map(StyleInput::Class)
    }

    /// Lowers a `.for(key)` use for `receiver`. A receiver with several candidates
    /// selects its transition at runtime.
    fn view_transition_receiver_use(
        &mut self,
        receiver: &Expression<'a>,
        key: &ViewTransitionKey,
    ) -> Option<StyleInput> {
        let transitions = match self.view_transition_references(receiver) {
            Ok(transitions) => transitions,
            Err(failure) => {
                self.report_resolution_failure(
                    receiver,
                    failure,
                    LintDiagnostic::InvalidViewTransitionReceiver,
                    CompileDiagnostic::CheckerFactsRequired {
                        subject: CheckerFactsSubject::ViewTransitionReceiver,
                    },
                );
                return None;
            }
        };
        let value = self.source_span(receiver.span());
        transition_use(
            value,
            transitions,
            ViewTransitionName::Key(key.clone()),
            false,
        )
    }

    /// Reads a use-site key. A literal key folds at compile time and any other
    /// expression is read by the runtime. More than one argument or a spread argument
    /// adds a lint and lowers as the empty key of `.for()`.
    ///
    /// Returns `None` after adding a fatal diagnostic when the source cannot be read.
    fn view_transition_key(&mut self, call: &CallExpression<'a>) -> Option<ViewTransitionKey> {
        let argument = match call.arguments.as_slice() {
            [argument] => argument.as_expression(),
            _ => None,
        };
        let Some(argument) = argument else {
            if !call.arguments.is_empty() {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidViewTransitionKeyExpression,
                    self.source_span(call.span),
                ));
            }
            return Some(ViewTransitionKey::Literal(LeanStr::new()));
        };
        match argument {
            Expression::StringLiteral(value) => {
                Some(ViewTransitionKey::Literal(self.string_literal(value).value))
            }
            Expression::NumericLiteral(value) => {
                let raw = match self.source.slice(self.source_span(value.span())) {
                    Ok(raw) => raw,
                    Err(error) => {
                        self.diagnostics.push(CompileError::new(
                            CompileDiagnostic::source_failure(
                                &error,
                                SourceFailureContext::Frontend,
                            ),
                            self.source_span(call.span),
                        ));
                        return None;
                    }
                };
                Some(ViewTransitionKey::Literal(raw.to_lean_str()))
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let cooked =
                    self.unicode_string(template.quasis[0].value.cooked?, template.span)?;
                Some(ViewTransitionKey::Literal(cooked.to_lean_str()))
            }
            argument => Some(ViewTransitionKey::Dynamic(
                self.source_span(argument.span()),
            )),
        }
    }

    /// Validates and records one `style.viewTransition({...})` declaration.
    ///
    /// Only `label`, `group`, `old`, `new` and `imagePair` are accepted, each with a
    /// literal string. Unsupported properties are reported and skipped. An empty
    /// definition retains its binding and identity.
    pub(crate) fn collect_view_transition_definition(
        &mut self,
        binding: Spanned<LeanStr>,
        call: &CallExpression<'a>,
    ) -> ViewTransitionDefinition {
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
                LintDiagnostic::InvalidViewTransitionDefinition,
                self.source_span(call.span),
            ));
        }
        let mut label = None;
        let mut group = None;
        let mut old = None;
        let mut new = None;
        let mut image_pair = None;
        for property in object.into_iter().flat_map(|object| &object.properties) {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::ViewTransitionSpread,
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidViewTransitionProperty,
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(property_name) = self.property_name(&property.key) else {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidViewTransitionKey,
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let Expression::StringLiteral(value) = &property.value else {
                let kind = if property_name.value == "label" {
                    LintDiagnostic::InvalidViewTransitionLabel
                } else {
                    LintDiagnostic::InvalidViewTransitionValue
                };
                self.lint_findings.push(LintFinding::new(
                    kind,
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            let value = self.string_literal(value);
            match property_name.value.as_str() {
                "label" => {
                    label = Some(value);
                }
                "group" => {
                    group = Some(value);
                }
                "old" => {
                    old = Some(value);
                }
                "new" => {
                    new = Some(value);
                }
                "imagePair" => {
                    image_pair = Some(value);
                }
                _ => {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::InvalidViewTransitionKey,
                        property_name.span,
                    ));
                }
            }
        }
        ViewTransitionDefinition {
            binding,
            call_span: self.source_span(call.span),
            label,
            group,
            old,
            new,
            image_pair,
        }
    }
}

/// Builds a use for the resolved `transitions` at `value`.
///
/// `dynamic` keeps a single candidate dynamic when the checker described a use without
/// a key. Returns `None` for an empty finite candidate set.
fn transition_use(
    value: Span,
    transitions: Vec<AssetReference>,
    name: ViewTransitionName,
    dynamic: bool,
) -> Option<StyleInput> {
    if dynamic && matches!(name, ViewTransitionName::None) {
        return (!transitions.is_empty()).then_some(StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        });
    }
    match transitions.as_slice() {
        [transition] => Some(StyleInput::ViewTransitionUse {
            transition: transition.clone(),
            name,
        }),
        [] => None,
        _ => Some(StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        }),
    }
}
