//! Omission of style inputs that contain other macro calls.
//!
//! Lowering keeps some parts of a style input as JavaScript: conditional tests, class
//! values, attribute and custom-property values, composition bases, and other
//! expressions that it does not lower into rules. The replacement of the call copies
//! their source text. A macro call inside such a part would stay in the generated code
//! without its own lowering, so the smallest input that contains it is omitted.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::ast_visit::{Visit, walk};
use oxc::span::{GetSpan, Span as OxcSpan};
use oxc::syntax::operator::LogicalOperator;

use stillwind_diagnostics::{LintDiagnostic, LintFinding};

use crate::state::MacroExtractor;

/// Macro members whose calls have their own lowering, with their display spellings.
const MACRO_MEMBERS: [(&str, &str); 6] = [
    ("cva", "style.cva()"),
    ("ref", "style.ref()"),
    ("keyframes", "style.keyframes()"),
    ("viewTransition", "style.viewTransition()"),
    ("media", "style.media()"),
    ("css", "style.css()"),
];

impl<'a> MacroExtractor<'a> {
    /// Omits `expression` when a part that lowering keeps as JavaScript contains a macro
    /// call. Returns whether it did.
    ///
    /// Arrays, objects, and conditional branches are lowered separately, so only the
    /// test of a conditional is checked for them. Any other expression is kept whole.
    pub(super) fn omit_kept_macros(&mut self, expression: &Expression<'a>) -> bool {
        let kept = match expression {
            Expression::ArrayExpression(_)
            | Expression::ObjectExpression(_)
            | Expression::StringLiteral(_)
            | Expression::ParenthesizedExpression(_) => return false,
            Expression::ConditionalExpression(conditional) => &conditional.test,
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
                &logical.left
            }
            _ => expression,
        };
        self.omit_nested_macro(kept, expression.span())
    }

    /// Omits `owner` and reports the first macro call in `expression`, in source order.
    /// Returns whether `expression` contains one.
    ///
    /// `owner` is the smallest input whose omission removes `expression`, such as a
    /// whole conditional for its test or an object property for its value. A `style()`
    /// call reports [`LintDiagnostic::NestedStyleCall`], and other macros report
    /// [`LintDiagnostic::NestedMacroCall`]. Calls inside functions count, because the
    /// copied source contains them too.
    pub(super) fn omit_nested_macro(
        &mut self,
        expression: &Expression<'a>,
        owner: OxcSpan,
    ) -> bool {
        let mut finder = MacroCallFinder {
            extractor: self,
            found: None,
        };
        finder.visit_expression(expression);
        let Some((call, syntax)) = finder.found else {
            return false;
        };
        self.omit_input(owner);
        let diagnostic = match syntax {
            None => LintDiagnostic::NestedStyleCall,
            Some(syntax) => LintDiagnostic::NestedMacroCall {
                syntax: syntax.into(),
            },
        };
        self.lint_findings
            .push(LintFinding::new(diagnostic, self.source_span(call)));
        true
    }
}

/// Finds the first macro call in an expression.
struct MacroCallFinder<'e, 'a> {
    extractor: &'e MacroExtractor<'a>,
    /// Span of the first call, with the display spelling of a macro member or `None`
    /// for `style()`.
    found: Option<(OxcSpan, Option<&'static str>)>,
}

impl<'a> Visit<'a> for MacroCallFinder<'_, 'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.found.is_some() {
            return;
        }
        if self.extractor.is_style_call(call) {
            self.found = Some((call.span, None));
            return;
        }
        let callee = call.callee.get_inner_expression();
        if let Some((_, syntax)) = MACRO_MEMBERS
            .iter()
            .find(|(member, _)| self.extractor.is_style_member(callee, member))
        {
            self.found = Some((call.span, Some(syntax)));
            return;
        }
        walk::walk_call_expression(self, call);
    }
}
