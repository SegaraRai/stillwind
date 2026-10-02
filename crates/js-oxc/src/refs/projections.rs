//! Parses ref provider and consumer projections and finite anchor-scope lists.
//!
//! The extractor methods recognize uses in supported style positions, resolve finite
//! ref candidates, and retain their original source coordinates in IR. They record
//! consumed macro ranges and checker probes; lowered style analysis checks attachments.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{AnchorScope, RefDefinition, RefProjection, RuleExpression, StyleInput};
use stillwind_source::Span;

use crate::local_span::LocalSpan;
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
    /// Returns `None` for unrelated syntax or invalid uses. An invalid use adds a
    /// diagnostic. The extractor may have changed even when the result is `None`.
    pub(crate) fn collect_ref_projection(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let span = self.source_span(expression.span());
        if let Expression::CallExpression(call) = expression
            && self.is_style_member(&call.callee, "anchorScope")
        {
            self.consumed_use_spans
                .insert(LocalSpan::from(expression.span()));
            let [argument] = call.arguments.as_slice() else {
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidAnchorScope,
                    span,
                ));
                return None;
            };
            let scope = self.collect_anchor_scope(argument.as_expression(), span)?;
            self.consumed_macro_uses.push(span);
            return Some(StyleInput::Class(
                RuleExpression::AnchorScope { scope, span }.into(),
            ));
        }
        let (receiver, projection) = match expression {
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                self.record_type_probe(&member.object);
                let projection = match member.property.name.as_str() {
                    "asAnchor" => {
                        self.consumed_use_spans.insert(LocalSpan::from(call.span));
                        let scope = match call.arguments.as_slice() {
                            [] => None,
                            [argument] => {
                                Some(self.collect_anchor_scope(argument.as_expression(), span)?)
                            }
                            _ => {
                                self.diagnostics.push(CompileError::new(
                                    CompileDiagnostic::InvalidAnchorScope,
                                    span,
                                ));
                                return None;
                            }
                        };
                        RefProjection::AsAnchor(scope)
                    }
                    "asContainer" => {
                        self.consumed_use_spans.insert(LocalSpan::from(call.span));
                        self.collect_container_type(call)?
                    }
                    "positionAnchor" => {
                        self.consumed_use_spans.insert(LocalSpan::from(call.span));
                        if !call.arguments.is_empty() {
                            self.diagnostics.push(CompileError::new(
                                CompileDiagnostic::InvalidPositionAnchorArguments,
                                span,
                            ));
                            return None;
                        }
                        RefProjection::PositionAnchor
                    }
                    _ => return None,
                };
                (&member.object, projection)
            }
            _ => return None,
        };
        let Some(refs) = self.ref_references(receiver) else {
            self.diagnostics.push(CompileError::new(
                CompileDiagnostic::RefProjectionFactsRequired,
                span,
            ));
            return None;
        };
        let value = self.source_span(receiver.span());
        Some(StyleInput::Class(
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
        ))
    }

    /// Parses the optional argument of an `asContainer` call.
    /// With no argument, the result is `Some(AsContainer(None))`. A single string literal
    /// is trimmed and normalized while keeping its source span.
    ///
    /// Returns `None` and adds an invalid-container-type diagnostic for too many arguments,
    /// a spread, a nonliteral argument, or unsupported text.
    fn collect_container_type(&mut self, call: &CallExpression<'a>) -> Option<RefProjection> {
        match call.arguments.as_slice() {
            [] => Some(RefProjection::AsContainer(None)),
            [argument] => {
                if let Some(Expression::StringLiteral(value)) = argument.as_expression() {
                    let mut kind = self.string_literal(value);
                    if let Some(normalized) = normalize_container_type(kind.value.trim()) {
                        kind.value = normalized.to_owned();
                        return Some(RefProjection::AsContainer(Some(kind)));
                    }
                }
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidContainerType,
                    self.source_span(argument.span()),
                ));
                None
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidContainerType,
                    self.source_span(call.span),
                ));
                None
            }
        }
    }

    /// Parses a scope keyword or literal ref array at the supplied source span.
    /// Each array entry is resolved to every possible ref declaration. Receiver probes
    /// are recorded for all entries even if another entry fails.
    ///
    /// The result contains each declaration identity once, in first-seen order. Empty
    /// arrays and duplicate identities add lint findings. An empty array produces an
    /// empty scope. The receiver is included only if the array lists it.
    ///
    /// Missing or invalid syntax adds a diagnostic and returns `None`.
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
                                .and_then(|value| self.ref_references(value)),
                        )
                    })
                    .collect::<Vec<_>>();
                for (element, candidates) in candidates {
                    let Some(candidates) = candidates else {
                        self.diagnostics.push(CompileError::new(
                            CompileDiagnostic::InvalidAnchorScope,
                            self.source_span(element.span()),
                        ));
                        return None;
                    };
                    for definition in candidates {
                        if refs.iter().any(|existing| {
                            existing.module_id == definition.module_id
                                && existing.binding == definition.binding
                        }) {
                            self.lint_findings.push(LintFinding::new(
                                LintDiagnostic::AnchorScopeList {
                                    detail: "contains duplicate ref identities".into(),
                                },
                                self.source_span(element.span()),
                            ));
                        } else {
                            refs.push(definition);
                        }
                    }
                }
                if refs.is_empty() {
                    self.lint_findings.push(LintFinding::new(
                        LintDiagnostic::AnchorScopeList {
                            detail: "is empty; no scope declaration is emitted".into(),
                        },
                        span,
                    ));
                }
                Some(AnchorScope::Refs(refs))
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileDiagnostic::InvalidAnchorScope,
                    span,
                ));
                None
            }
        }
    }
}

/// Returns a supported container type with its keywords in canonical order.
/// The caller must trim surrounding whitespace. Internal spacing must match a
/// supported spelling. Unsupported text returns `None` without adding a diagnostic.
fn normalize_container_type(value: &str) -> Option<&str> {
    match value {
        "scroll-state size" => Some("size scroll-state"),
        "scroll-state inline-size" => Some("inline-size scroll-state"),
        "normal"
        | "size"
        | "inline-size"
        | "scroll-state"
        | "size scroll-state"
        | "inline-size scroll-state"
        | "initial"
        | "inherit"
        | "unset"
        | "revert"
        | "revert-layer"
        | "revert-rule" => Some(value),
        _ => None,
    }
}
