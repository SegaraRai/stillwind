//! Ref projections: `anchor`, `position`, `scope`, `container` and `at`.
//!
//! A projection turns a ref identity into something a style call can attach to an
//! element or use as a container query source. Projections need the ref to be
//! resolvable to finite candidates, from local analysis or checker facts. Unresolved
//! refs are reported as `SHTS001` instead of being guessed.
//!
//! Every use is also recorded with the JSX path it appeared on, which is what the
//! ancestry validation later reads. A projection over several candidates becomes an
//! explicitly dynamic projection instead of one candidate.

use oxc::ast::ast::Expression;
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{ClassExpression, RefDefinition, RefProjection, ScopeDefinition, StyleInput};
use stillwind_source::Span;

use crate::state::MacroExtractor;

/// One recorded ref use.
#[derive(Clone, Debug)]
pub(crate) struct RefUse {
    /// Marker name of the ref that was used, as written at its declaration.
    pub(crate) name: String,
    /// Use kind: `ref`, `selector`, `anchor`, `position`, `scope`, `container` or `container-query`.
    pub(crate) projection: &'static str,
    /// Whole expression the use came from.
    pub(crate) span: Span,
    /// JSX ancestor path of the use, when it appeared inside JSX.
    pub(crate) jsx_path: Option<Vec<usize>>,
}

/// One recorded scope use.
#[derive(Clone, Debug)]
pub(crate) struct ScopeUse {
    /// Marker name of the scope that was used.
    pub(crate) name: String,
    /// JSX ancestor path of the use, when it appeared inside JSX.
    pub(crate) jsx_path: Option<Vec<usize>>,
}

impl<'a> MacroExtractor<'a> {
    /// Lowers one ref projection expression, if it is one.
    ///
    /// Recognizes the member projections and `ref.container("type")`, and returns
    /// `None` when the expression is not a projection so callers can try other forms.
    /// Arguments must be literal, because the projection becomes generated CSS rather
    /// than a runtime value.
    pub(crate) fn collect_ref_projection(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        let (ref_expression, projection) = match expression {
            Expression::StaticMemberExpression(member) => {
                let projection = match member.property.name.as_str() {
                    "anchor" => RefProjection::Anchor,
                    "position" => RefProjection::Position,
                    "anchorScope" => RefProjection::Scope,
                    _ => return None,
                };
                (&member.object, projection)
            }
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                if member.property.name.as_str() != "container" {
                    return None;
                }
                let [argument] = call.arguments.as_slice() else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidContainerType,
                        self.source_span(call.span),
                    ));
                    return None;
                };
                let Some(Expression::StringLiteral(value)) = argument.as_expression() else {
                    self.diagnostics.push(CompileError::new(
                        CompileErrorKind::InvalidContainerType,
                        self.source_span(argument.span()),
                    ));
                    return None;
                };
                (
                    &member.object,
                    RefProjection::Container(self.string_literal(value)),
                )
            }
            _ => return None,
        };
        let Some(refs) = self.ref_references(ref_expression) else {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::RefProjectionFactsRequired,
                self.source_span(expression.span()),
            ));
            return None;
        };
        let projection_name = match &projection {
            RefProjection::Anchor => "anchor",
            RefProjection::Position => "position",
            RefProjection::Scope => "scope",
            RefProjection::Container(_) => "container",
        };
        for definition in &refs {
            self.record_ref_use(
                definition,
                projection_name,
                self.source_span(expression.span()),
            );
        }
        if refs.len() == 1 {
            Some(StyleInput::Class(ClassExpression::RefProjection {
                definition: refs[0].clone(),
                value: self.source_span(ref_expression.span()),
                projection,
            }))
        } else {
            Some(StyleInput::Class(ClassExpression::DynamicRefProjection {
                value: self.source_span(ref_expression.span()),
                refs,
                projection,
            }))
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
        projection: &'static str,
        span: Span,
    ) {
        self.ref_uses.push(RefUse {
            name: definition.binding.value.clone(),
            projection,
            span,
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }

    /// Records one scope use, which makes the scope a provider for its JSX path.
    pub(crate) fn record_scope_use(&mut self, scope: &ScopeDefinition) {
        self.scope_uses.push(ScopeUse {
            name: scope.binding.value.clone(),
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }
}
