//! Syntactic effect proofs used to relax generated-expression ordering.
//!
//! [`collect`] checks only expression spans already referenced by the extracted IR.
//! Calls/new require PURE annotations, and explicit writes or control effects remain
//! effectful. Annotated calls still inspect their callee and arguments; creating a
//! function does not inspect its unexecuted parameters or body.
//!
//! Getter, proxy, iterator, implicit-coercion, and reference-error behavior is outside
//! this heuristic. It performs no call-graph or interprocedural analysis.

use std::collections::BTreeSet;

use oxc::ast::ast::{CallExpression, Expression, Function, Program};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan;
use oxc::syntax::scope::ScopeFlags;

use stillwind_ir::ModuleIr;
use stillwind_source::{FileId, Span};

/// Returns distinct IR-referenced expression spans passing the syntactic effect check.
///
/// `program`, `module` spans, and `file` must address the same parsed buffer. Results
/// follow AST visit order and use `file`; non-expression spans and unproven expressions
/// are omitted. Existing purity entries do not bypass analysis, and neither input is
/// mutated. Projection to editable source coordinates belongs to the caller.
pub(crate) fn collect(program: &Program<'_>, module: &ModuleIr, file: FileId) -> Vec<Span> {
    let mut wanted = BTreeSet::new();
    let _ = module.try_map_spans(|span| {
        wanted.insert((span.start.0, span.end.0));
        Ok::<_, std::convert::Infallible>(span)
    });
    let mut collector = Collector {
        wanted,
        pure: Vec::new(),
        file,
    };
    collector.visit_program(program);
    collector.pure
}

/// Visits all AST scopes while checking only requested expression coordinates.
struct Collector {
    /// Unvisited `(start, end)` pairs in the parsed buffer, measured in UTF-8 bytes.
    wanted: BTreeSet<(u32, u32)>,
    /// Proven expression occurrences in AST encounter order.
    pure: Vec<Span>,
    /// Source identity assigned to every emitted proof.
    file: FileId,
}

impl<'a> Visit<'a> for Collector {
    /// Consumes a requested span once, records a proof if available, and visits children.
    /// Traversal continues into functions so style calls inside them can obtain proofs.
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        let span = expression.span();
        if self.wanted.remove(&(span.start, span.end)) {
            let mut effects = Effects(false);
            effects.visit_expression(expression);
            if !effects.0 {
                self.pure.push(Span::new(self.file, span.start, span.end));
            }
        }
        walk::walk_expression(self, expression);
    }
}

/// Monotonic result of inspecting the evaluation of one expression.
struct Effects(
    /// `true` once a potentially effectful construct is found; never reset during a walk.
    bool,
);

impl<'a> Visit<'a> for Effects {
    /// Skips function parameters and bodies, which are not evaluated by function creation.
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}

    /// Marks unannotated calls effectful, including optional-chain calls.
    /// PURE calls still visit callee and argument evaluations without clearing prior effects.
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if call.pure {
            walk::walk_call_expression(self, call);
        } else {
            self.0 = true;
        }
    }

    /// Accumulates syntactic effects, stopping once one is known.
    /// Function/arrow creation is inert; other expressions recurse unless explicitly
    /// classified as effectful, including class/JSX initialization and suspension.
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if self.0 {
            return;
        }
        match expression {
            // Function creation does not execute its parameters or body.
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {},
            Expression::CallExpression(call) if !call.pure => self.0 = true,
            Expression::NewExpression(call) if !call.pure => self.0 = true,
            Expression::AssignmentExpression(_) | Expression::UpdateExpression(_)
            | Expression::AwaitExpression(_) | Expression::YieldExpression(_)
            | Expression::TaggedTemplateExpression(_) | Expression::ImportExpression(_)
            // Class initialization can execute static blocks and decorators.
            | Expression::ClassExpression(_) | Expression::JSXElement(_)
            | Expression::JSXFragment(_) => self.0 = true,
            Expression::UnaryExpression(unary) if unary.operator.is_delete() => self.0 = true,
            _ => walk::walk_expression(self, expression),
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc::{allocator::Allocator, parser::Parser, span::SourceType};

    use super::*;

    /// Parses an expression as TSX and returns the heuristic's effect-free result.
    ///
    /// # Panics
    ///
    /// Panics if the supplied test expression is not valid TSX syntax.
    fn pure(text: &str) -> bool {
        let allocator = Allocator::default();
        let source = format!("({text});");
        let parsed = Parser::new(&allocator, &source, SourceType::tsx()).parse();
        assert!(
            parsed.diagnostics.is_empty(),
            "{text}: {:?}",
            parsed.diagnostics
        );
        let mut effects = Effects(false);
        effects.visit_program(&parsed.program);
        !effects.0
    }

    /// Contrasts inert function creation and getter reads with executed calls and writes.
    #[test]
    fn pure_calls_still_inspect_executed_arguments_and_callees() {
        for expression in [
            "value",
            "this.value",
            "object.value",
            "object[key]",
            "object?.value",
            "a + b",
            "flag ? a : b",
            "`value:${value}`",
            "value as number",
            "/*#__PURE__*/ f(value)",
            "/* @__PURE__ */ new Value(value)",
            "/*#__PURE__*/ f(/*#__PURE__*/ g())",
            "() => f()",
            "function (value = f()) { g(); }",
            "({ get value() { return f(); } })",
        ] {
            assert!(pure(expression), "{expression}");
        }
        for expression in [
            "f()",
            "f?.()",
            "object?.f()",
            "new Value()",
            "object[f()]",
            "/*#__PURE__*/ f(g())",
            "/*#__PURE__*/ new Value(g())",
            "/*#__PURE__*/ (getFactory())(value)",
            "/*#__PURE__*/ f(value++)",
            "value = other",
            "++value",
            "delete object.value",
            "tag`text`",
            "import('module')",
            "class { static { f(); } }",
        ] {
            assert!(!pure(expression), "{expression}");
        }
    }
}
