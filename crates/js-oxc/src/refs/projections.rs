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

use oxc::ast::ast::{CallExpression, Expression, StaticMemberExpression};
use oxc::span::GetSpan;

use stillwind_ir::{
    ClassExpression, HandleDefinition, HandleProjection, ScopeDefinition, StyleInput,
};
use stillwind_source::{CompileError, Span};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Lowers one ref projection expression, if it is one.
    ///
    /// Recognizes the member projections and `ref.container("type")`, and returns
    /// `None` when the expression is not a projection so callers can try other forms.
    /// Arguments must be literal, because the projection becomes generated CSS rather
    /// than a runtime value.
    pub(crate) fn collect_handle_projection(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<StyleInput> {
        if let Expression::CallExpression(call) = expression
            && let Expression::StaticMemberExpression(member) = &call.callee
            && member.property.name.as_str() == "at"
        {
            return self.collect_container_query_projection(call, member);
        }
        let (handle_expression, projection) = match expression {
            Expression::StaticMemberExpression(member) => {
                let projection = match member.property.name.as_str() {
                    "anchor" => HandleProjection::Anchor,
                    "position" => HandleProjection::Position,
                    "anchorScope" => HandleProjection::Scope,
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
                let Some(argument) = call.arguments.first() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
                        self.source_span(call.span),
                    ));
                    return None;
                };
                let Some(Expression::StringLiteral(value)) = argument.as_expression() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
                        self.source_span(argument.span()),
                    ));
                    return None;
                };
                (
                    &member.object,
                    HandleProjection::Container(self.string_literal(value)),
                )
            }
            _ => return None,
        };
        let Some(handles) = self.handle_references(handle_expression) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional ref projections require TypeScript checker facts",
                self.source_span(expression.span()),
            ));
            return None;
        };
        let projection_name = match &projection {
            HandleProjection::Anchor => "anchor",
            HandleProjection::Position => "position",
            HandleProjection::Scope => "scope",
            HandleProjection::Container(_) => "container",
        };
        for handle in &handles {
            self.record_handle_use(handle, projection_name, self.source_span(expression.span()));
        }
        if handles.len() == 1 {
            Some(StyleInput::Class(ClassExpression::HandleProjection {
                handle: handles[0].clone(),
                projection,
            }))
        } else {
            Some(StyleInput::Class(
                ClassExpression::DynamicHandleProjection {
                    value: self.source_span(expression.span()),
                    handles,
                    projection,
                },
            ))
        }
    }

    /// Lowers `ref.at("query", "utilities")` into a container query input.
    ///
    /// Both arguments must be literals and the receiver must resolve to a single ref,
    /// so the query can be emitted as CSS instead of evaluated at runtime.
    pub(crate) fn collect_container_query_projection(
        &mut self,
        call: &CallExpression<'a>,
        member: &StaticMemberExpression<'a>,
    ) -> Option<StyleInput> {
        if call.arguments.len() != 2 {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(handle) = self.handle_reference(&member.object) else {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "ref projections must use an identity returned by style.ref()",
                self.source_span(call.span),
            ));
            return None;
        };
        let (Some(Expression::StringLiteral(query)), Some(Expression::StringLiteral(value))) = (
            call.arguments[0].as_expression(),
            call.arguments[1].as_expression(),
        ) else {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        };
        self.record_handle_use(&handle, "container-query", self.source_span(call.span));
        Some(StyleInput::ContainerQuery {
            handle,
            query: self.string_literal(query),
            value: self.string_literal(value),
        })
    }

    /// Records one ref use for later ancestry validation.
    ///
    /// `projection` is the use kind that `validation` reads and `span` is the whole
    /// expression the user wrote. The JSX path is recorded only when the use is inside
    /// a JSX tree, because the ancestry rules cannot be checked otherwise.
    pub(crate) fn record_handle_use(
        &mut self,
        handle: &HandleDefinition,
        projection: &'static str,
        span: Span,
    ) {
        self.handle_uses.push(HandleUse {
            name: handle.name.value.clone(),
            projection,
            span,
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }

    /// Records one scope use, which makes the scope a provider for its JSX path.
    pub(crate) fn record_scope_use(&mut self, scope: &ScopeDefinition) {
        self.scope_uses.push(ScopeUse {
            name: scope.name.value.clone(),
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }
}

/// One recorded ref use.
#[derive(Clone, Debug)]
pub(crate) struct HandleUse {
    /// Marker name of the ref that was used, as written at its declaration.
    pub(crate) name: String,
    /// Use kind: `ref`, `selector`, `anchor`, `position`, `scope`, `container` or
    /// `container-query`.
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
