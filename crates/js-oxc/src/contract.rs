//! Checks syntactically visible violations of the macro value-expression contract.
//!
//! [`validate`] visits extracted macro calls in their original AST. Ordinary calls and
//! constructors rely on the caller's purity contract. Function bodies belong to that
//! contract and are not analyzed interprocedurally; nested macros are checked separately.

use std::collections::BTreeSet;

use oxc::ast::ast::{CallExpression, Expression, Function, Program};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan;
use oxc::syntax::scope::ScopeFlags;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::ModuleIr;
use stillwind_source::{FileId, Span};

/// Returns fatal contract violations at original-source UTF-8 spans.
/// `program` and `module` must describe the same buffer identified by `file`.
/// Only extracted macro calls are checked; absence of a diagnostic does not prove
/// purity of opaque callees, getters, conversions, or aliases of Hooks.
pub(crate) fn validate(
    program: &Program<'_>,
    module: &ModuleIr,
    file: FileId,
) -> Vec<CompileError> {
    let roots = module
        .style_calls
        .iter()
        .map(|call| call.span)
        .chain(module.handle_definitions.iter().map(|item| item.call_span))
        .chain(module.scope_definitions.iter().map(|item| item.call_span))
        .chain(
            module
                .animation_declarations
                .iter()
                .map(|item| item.call_span),
        )
        .chain(
            module
                .view_transition_declarations
                .iter()
                .map(|item| item.call_span),
        )
        .map(|span| (span.start.0, span.end.0))
        .collect();
    let mut collector = Collector {
        roots,
        violations: Violations {
            file,
            errors: Vec::new(),
        },
    };
    collector.visit_program(program);
    collector
        .violations
        .errors
        .sort_by_key(|error| error.labels.first().map(|label| label.span.start));
    collector
        .violations
        .errors
        .dedup_by(|a, b| a.kind == b.kind && a.labels == b.labels);
    collector.violations.errors
}

/// Locates macro roots while still traversing functions containing independent macros.
struct Collector {
    /// Unvisited original-buffer UTF-8 call ranges.
    roots: BTreeSet<(u32, u32)>,
    violations: Violations,
}

impl<'a> Visit<'a> for Collector {
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        let span = expression.span();
        if self.roots.remove(&(span.start, span.end)) {
            self.violations.visit_expression(expression);
        }
        walk::walk_expression(self, expression);
    }
}

/// Checks one value-expression evaluation without entering opaque function bodies.
struct Violations {
    /// Source identity shared by the AST and every reported label.
    file: FileId,
    errors: Vec<CompileError>,
}

impl Violations {
    fn report(&mut self, span: oxc::span::Span, kind: CompileErrorKind) {
        self.errors.push(CompileError::new(
            kind,
            Span::new(self.file, span.start, span.end),
        ));
    }
}

impl<'a> Visit<'a> for Violations {
    /// Local mutation inside a function belongs to its caller-supplied purity contract.
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let name = match call.callee.get_inner_expression() {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
            Expression::ComputedMemberExpression(member) => {
                match member.expression.get_inner_expression() {
                    Expression::StringLiteral(name) => Some(name.value.as_str()),
                    _ => None,
                }
            }
            _ => None,
        };
        if name.is_some_and(|name| {
            name == "use"
                || name
                    .strip_prefix("use")
                    .is_some_and(|tail| tail.starts_with(|c: char| c.is_ascii_uppercase()))
        }) {
            self.report(call.span, CompileErrorKind::MacroExecutionBoundary);
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        match expression {
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
            Expression::AssignmentExpression(_) | Expression::UpdateExpression(_) => {
                self.report(expression.span(), CompileErrorKind::MacroArgumentMutation);
            }
            Expression::UnaryExpression(unary) if unary.operator.is_delete() => {
                self.report(expression.span(), CompileErrorKind::MacroArgumentMutation);
            }
            Expression::AwaitExpression(_)
            | Expression::YieldExpression(_)
            | Expression::ImportExpression(_)
            | Expression::ClassExpression(_)
            | Expression::JSXElement(_)
            | Expression::JSXFragment(_) => {
                self.report(expression.span(), CompileErrorKind::MacroExecutionBoundary);
            }
            _ => walk::walk_expression(self, expression),
        }
    }
}
