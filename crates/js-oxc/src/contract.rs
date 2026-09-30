//! Checks syntactically visible violations of the macro value-expression contract.
//!
//! [`validate`] visits extracted macro calls in their original AST. Ordinary calls and
//! constructors rely on the caller's purity contract. Function bodies belong to that
//! contract and are not analyzed interprocedurally; nested macros are checked separately.

use std::collections::BTreeSet;

use oxc::ast::ast::{Expression, Function, Program};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan;
use oxc::syntax::scope::ScopeFlags;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::ModuleIr;
use stillwind_source::{FileId, Span};

use crate::local_span::LocalSpan;

/// Returns suppressible contract findings at original-source UTF-8 spans.
/// `program` and `module` must describe the same buffer identified by `file`.
/// Only extracted macro calls are checked; absence of a diagnostic does not prove
/// purity of opaque callees, getters, or conversions.
pub(crate) fn validate(program: &Program<'_>, module: &ModuleIr, file: FileId) -> Vec<LintFinding> {
    let roots = module
        .style_calls
        .iter()
        .map(|call| call.span)
        .chain(module.ref_definitions.iter().map(|item| item.call_span))
        .chain(
            module
                .keyframes_declarations
                .iter()
                .map(|item| item.call_span),
        )
        .chain(
            module
                .view_transition_declarations
                .iter()
                .map(|item| item.call_span),
        )
        .map(|span| LocalSpan::new(span.start.0, span.end.0))
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
        .dedup_by(|a, b| a.diagnostic == b.diagnostic && a.labels == b.labels);
    collector.violations.errors
}

/// Locates macro roots while still traversing functions containing independent macros.
struct Collector {
    /// Unvisited original-buffer UTF-8 call ranges.
    roots: BTreeSet<LocalSpan>,
    violations: Violations,
}

impl<'a> Visit<'a> for Collector {
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        let span = expression.span();
        if self.roots.remove(&LocalSpan::from(span)) {
            self.violations.visit_expression(expression);
        }
        walk::walk_expression(self, expression);
    }
}

/// Checks one value-expression evaluation without entering opaque function bodies.
struct Violations {
    /// Source identity shared by the AST and every reported label.
    file: FileId,
    errors: Vec<LintFinding>,
}

impl Violations {
    fn report(&mut self, span: oxc::span::Span, diagnostic: LintDiagnostic) {
        self.errors.push(LintFinding::new(
            diagnostic,
            Span::new(self.file, span.start, span.end),
        ));
    }
}

impl<'a> Visit<'a> for Violations {
    /// Local mutation inside a function belongs to its caller-supplied purity contract.
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        match expression {
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
            Expression::AssignmentExpression(_) | Expression::UpdateExpression(_) => {
                self.report(expression.span(), LintDiagnostic::MacroArgumentMutation);
            }
            Expression::UnaryExpression(unary) if unary.operator.is_delete() => {
                self.report(expression.span(), LintDiagnostic::MacroArgumentMutation);
            }
            Expression::AwaitExpression(_)
            | Expression::YieldExpression(_)
            | Expression::ImportExpression(_) => {
                self.report(expression.span(), LintDiagnostic::MacroExecutionBoundary);
            }
            _ => walk::walk_expression(self, expression),
        }
    }
}
