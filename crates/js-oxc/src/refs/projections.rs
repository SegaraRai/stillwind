//! Parses ref provider and consumer projections and finite anchor-scope lists.
//!
//! The extractor methods recognize uses in supported style positions, resolve finite
//! ref candidates, and retain their original source coordinates in IR. They record
//! consumed macro ranges and checker probes; lowered style analysis checks attachments.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CheckerFactsSubject, CompileDiagnostic, LintDiagnostic, LintFinding};
use stillwind_ir::{
    AnchorScope, ClassExpression, ContainerType, RefDefinition, RefProjection, RuleExpression,
    StyleInput,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Parses an expression into a ref rule. The caller must pass an expression from
    /// a supported style input position.
    ///
    /// The parser resolves receivers and scope members to finite sets of declarations.
    /// It records checker probes, and marks recognized syntax as consumed before
    /// validating its arguments. Successful scope-only calls also record the ranges
    /// needed to remove their macro imports.
    ///
    /// An invalid argument adds a lint and lowers as an omitted argument. A receiver
    /// that [`needs_branch_lowering`](Self::needs_branch_lowering) lowers one
    /// projection per branch, and a branch that is not a ref adds a lint and
    /// contributes nothing.
    ///
    /// Returns `None` for unrelated syntax, for missing checker facts, and for a use
    /// that contributes nothing. The extractor may have changed even when the result is
    /// `None`.
    pub(crate) fn collect_ref_projection(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let span = self.source_span(expression.span());
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        if self.is_style_member(&call.callee, "anchorScope") {
            self.consumed_use_spans
                .insert(LocalSpan::from(expression.span()));
            self.consumed_macro_uses.push(span);
            let [argument] = call.arguments.as_slice() else {
                self.lint_findings
                    .push(LintFinding::new(LintDiagnostic::InvalidAnchorScope, span));
                return None;
            };
            let scope = self.collect_anchor_scope(argument.as_expression(), span)?;
            return Some(StyleInput::Class(
                RuleExpression::AnchorScope { scope, span }.into(),
            ));
        }
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        self.record_type_probe(&member.object);
        let projection = match member.property.name.as_str() {
            "asAnchor" => {
                self.consumed_use_spans.insert(LocalSpan::from(call.span));
                let scope = match call.arguments.as_slice() {
                    [] => None,
                    [argument] => self.collect_anchor_scope(argument.as_expression(), span),
                    _ => {
                        self.lint_findings
                            .push(LintFinding::new(LintDiagnostic::InvalidAnchorScope, span));
                        None
                    }
                };
                RefProjection::AsAnchor(scope)
            }
            "asContainer" => {
                self.consumed_use_spans.insert(LocalSpan::from(call.span));
                self.collect_container_type(call)
            }
            "positionAnchor" => {
                self.consumed_use_spans.insert(LocalSpan::from(call.span));
                if !call.arguments.is_empty() {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::InvalidPositionAnchorArguments,
                        span,
                    ));
                }
                RefProjection::PositionAnchor
            }
            _ => return None,
        };
        let receiver = &member.object;
        if let Err(failure) = self.ref_references(receiver) {
            self.report_resolution_failure(
                receiver,
                failure,
                LintDiagnostic::InvalidRefProjection,
                CompileDiagnostic::CheckerFactsRequired {
                    subject: CheckerFactsSubject::RefProjection,
                },
            );
            return None;
        }
        let class = if self.needs_branch_lowering(receiver, AssetKind::Ref) {
            self.lower_receiver_branches(receiver, &mut |this, receiver| {
                this.ref_projection(receiver, projection.clone())
            })
        } else {
            self.ref_projection(receiver, projection)
        };
        class.map(StyleInput::Class)
    }

    /// Lowers `projection` for `receiver`. A receiver with several candidates selects
    /// its ref at runtime. A receiver that is not a ref adds a lint and returns `None`.
    fn ref_projection(
        &mut self,
        receiver: &Expression<'a>,
        projection: RefProjection,
    ) -> Option<ClassExpression> {
        let refs = match self.ref_references(receiver) {
            Ok(refs) => refs,
            Err(failure) => {
                self.report_resolution_failure(
                    receiver,
                    failure,
                    LintDiagnostic::InvalidRefProjection,
                    CompileDiagnostic::CheckerFactsRequired {
                        subject: CheckerFactsSubject::RefProjection,
                    },
                );
                return None;
            }
        };
        let value = self.source_span(receiver.span());
        Some(
            if let [definition] = refs.as_slice() {
                RuleExpression::RefProjection {
                    definition: definition.clone(),
                    value,
                    projection,
                }
            } else {
                RuleExpression::DynamicRefProjection {
                    value,
                    refs,
                    projection,
                }
            }
            .into(),
        )
    }

    /// Parses the optional argument of an `asContainer` call.
    /// With no argument, the result is `AsContainer(None)`. A single string literal is
    /// trimmed and normalized while keeping its source span.
    ///
    /// Too many arguments, a spread, a nonliteral argument, or unsupported text adds an
    /// invalid-container-type lint and returns `AsContainer(None)`.
    fn collect_container_type(&mut self, call: &CallExpression<'a>) -> RefProjection {
        match call.arguments.as_slice() {
            [] => RefProjection::AsContainer(None),
            [argument] => {
                if let Some(Expression::StringLiteral(value)) = argument.as_expression() {
                    let kind = self.string_literal(value);
                    if let Some(normalized) = ContainerType::parse(kind.value.trim()) {
                        let kind = Spanned::new(normalized, kind.span);
                        return RefProjection::AsContainer(Some(kind));
                    }
                }
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidContainerType,
                    self.source_span(argument.span()),
                ));
                RefProjection::AsContainer(None)
            }
            _ => {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidContainerType,
                    self.source_span(call.span),
                ));
                RefProjection::AsContainer(None)
            }
        }
    }

    /// Parses a scope keyword or literal ref array at the supplied source span.
    /// Each array entry is resolved to every possible ref declaration. Receiver probes
    /// are recorded for all entries even if another entry fails.
    ///
    /// The result contains each declaration identity once, in first-seen order. Empty
    /// arrays and duplicate identities add lint findings. An empty array produces an
    /// empty scope. The receiver is included only if the array lists it. An entry that
    /// is not a ref adds a lint and contributes no identity.
    ///
    /// Returns `None` when the argument is not a keyword or an array, after adding a
    /// lint. Also returns `None` after adding a fatal diagnostic when an entry needs
    /// checker facts.
    fn collect_anchor_scope(
        &mut self,
        expression: Option<&Expression<'a>>,
        span: Span,
    ) -> Option<AnchorScope> {
        match expression.map(Expression::get_inner_expression) {
            Some(Expression::StringLiteral(value)) if value.value == "all" => {
                Some(AnchorScope::All)
            }
            Some(Expression::StringLiteral(value)) if value.value == "none" => {
                Some(AnchorScope::None)
            }
            Some(Expression::ArrayExpression(array)) => {
                let mut refs: Vec<RefDefinition> = Vec::new();
                let candidates = array
                    .elements
                    .iter()
                    .map(|element| {
                        (
                            element,
                            element
                                .as_expression()
                                .map_or(Err(ResolutionFailure::Unsupported), |value| {
                                    self.ref_references(value)
                                }),
                        )
                    })
                    .collect::<Vec<_>>();
                for (element, candidates) in candidates {
                    let candidates = match candidates {
                        Ok(candidates) => {
                            if let Some(expression) = element.as_expression() {
                                self.report_unsupported_branches(
                                    expression,
                                    AssetKind::Ref,
                                    &LintDiagnostic::InvalidAnchorScope,
                                );
                            }
                            candidates
                        }
                        Err(failure) => {
                            let missing = failure == ResolutionFailure::MissingFacts;
                            if let Some(expression) = element.as_expression() {
                                self.report_resolution_failure(
                                    expression,
                                    failure,
                                    LintDiagnostic::InvalidAnchorScope,
                                    CompileDiagnostic::CheckerFactsRequired {
                                        subject: CheckerFactsSubject::RefProjection,
                                    },
                                );
                            } else {
                                self.lint_findings.push(LintFinding::new(
                                    LintDiagnostic::InvalidAnchorScope,
                                    self.source_span(element.span()),
                                ));
                            }
                            if missing {
                                return None;
                            }
                            continue;
                        }
                    };
                    for definition in candidates {
                        if refs.iter().any(|existing| {
                            existing.module_id == definition.module_id
                                && existing.binding == definition.binding
                        }) {
                            if self.lint {
                                self.lint_findings.push(LintFinding::new(
                                    LintDiagnostic::IneffectiveAnchorScope {
                                        detail: "contains duplicate ref identities".into(),
                                    },
                                    self.source_span(element.span()),
                                ));
                            }
                        } else {
                            refs.push(definition);
                        }
                    }
                }
                if self.lint && refs.is_empty() {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::IneffectiveAnchorScope {
                            detail: "is empty; no scope declaration is emitted".into(),
                        },
                        span,
                    ));
                }
                Some(AnchorScope::Refs(refs))
            }
            _ => {
                self.lint_findings
                    .push(LintFinding::new(LintDiagnostic::InvalidAnchorScope, span));
                None
            }
        }
    }
}
