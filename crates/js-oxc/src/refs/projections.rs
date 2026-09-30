//! Explicit ref provider and consumer uses, finite scope lists, and recorded attachments.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileError, CompileErrorKind, LintFinding, LintKind};
use stillwind_ir::{AnchorScope, RefDefinition, RefProjection, RuleExpression, StyleInput};
use stillwind_source::{ModuleId, Span};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

/// Ref use kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RefUseKind {
    /// `ref` itself.
    Marker,
    /// `ref("hover:")` and `` ref`hover:` `` variants.
    SelectorVariant,
    /// `ref.asAnchor(...)`.
    AsAnchor,
    /// `ref.asContainer(...)`.
    AsContainer,
    /// `ref.positionAnchor`.
    PositionAnchor,
    /// `ref.at(...)` vatiant.
    ContainerQueryVariant,
}

impl<'a> From<&'a RefProjection> for RefUseKind {
    fn from(value: &'a RefProjection) -> Self {
        match value {
            RefProjection::AsAnchor(_) => RefUseKind::AsAnchor,
            RefProjection::AsContainer(_) => RefUseKind::AsContainer,
            RefProjection::PositionAnchor => RefUseKind::PositionAnchor,
        }
    }
}

/// One recorded ref use.
#[derive(Clone, Debug)]
pub(crate) struct RefUse {
    /// Marker name of the ref that was used, as written at its declaration.
    pub(crate) name: String,
    /// Declaration identity used to distinguish aliases from equal binding spellings.
    pub(crate) identity: (ModuleId, String, Span),
    /// Use kind.
    pub(crate) projection: RefUseKind,
    /// Whole expression the use came from.
    pub(crate) span: Span,
    /// JSX ancestor path of the use, when it appeared inside JSX.
    pub(crate) jsx_path: Option<Vec<usize>>,
}

impl<'a> MacroExtractor<'a> {
    /// Resolves explicit provider/consumer uses and scope-only uses to finite identities.
    /// Returns `None` for unrelated expressions or after reporting invalid arguments.
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
                    CompileErrorKind::InvalidAnchorScope,
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
            Expression::StaticMemberExpression(member)
                if member.property.name == "positionAnchor" =>
            {
                self.consumed_use_spans
                    .insert(LocalSpan::from(expression.span()));
                (&member.object, RefProjection::PositionAnchor)
            }
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
                                    CompileErrorKind::InvalidAnchorScope,
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
                    _ => return None,
                };
                (&member.object, projection)
            }
            _ => return None,
        };
        let Some(refs) = self.ref_references(receiver) else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::RefProjectionFactsRequired,
                span,
            ));
            return None;
        };
        let kind = (&projection).into();
        for definition in &refs {
            self.record_ref_use(definition, kind, span);
        }
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

    /// Validates an optional literal container type and returns its canonical spelling.
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
                    CompileErrorKind::InvalidContainerType,
                    self.source_span(argument.span()),
                ));
                None
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidContainerType,
                    self.source_span(call.span),
                ));
                None
            }
        }
    }

    /// Resolves a scope list to its complete candidate union, retaining first occurrence order.
    /// Invalid entries are fatal diagnostics; empty and repeated identities produce lint findings.
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
                            CompileErrorKind::InvalidAnchorScope,
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
                                LintKind::AnchorScopeList {
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
                        LintKind::AnchorScopeList {
                            detail: "is empty; no scope declaration is emitted".into(),
                        },
                        span,
                    ));
                }
                Some(AnchorScope::Refs(refs))
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidAnchorScope,
                    span,
                ));
                None
            }
        }
    }

    /// Records one ref use for later ancestry validation.
    ///
    /// `projection` is the use kind that `validation` reads and `span` is the whole
    /// expression the user wrote. The JSX path is recorded only when the use is inside
    /// a JSX tree, because the ancestry rules cannot be checked otherwise.
    pub(crate) fn record_ref_use(
        &mut self,
        definition: &RefDefinition,
        projection: RefUseKind,
        span: Span,
    ) {
        self.ref_uses.push(RefUse {
            name: definition.binding.value.clone(),
            identity: (
                definition.module_id.clone(),
                definition.binding.value.clone(),
                definition.binding.span,
            ),
            projection,
            span,
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }
}

/// Canonicalizes the finite keyword grammar accepted by the public container-type API.
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
