//! Recognition of `style.forward()` arguments.
//!
//! A `style.forward(props)` call is accepted in two positions: as a direct argument of
//! `style()`, where it becomes a [`StyleInput::Forward`], and as the second argument of
//! another call, such as a `style.cva()` factory call, where it becomes a
//! [`ForwardCall`] whose callee lowering replaces with a runtime helper. Any other
//! position is reported as an unconsumed macro use when the visit reaches the call.

use oxc::ast::ast::{Argument, CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{ForwardCall, ForwardSupport, FrameworkSpec, StyleInput};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Returns the forward input when `expression` is a direct `style.forward(...)`
    /// argument of a `style()` call, or `None` when it is another expression.
    ///
    /// A recognized call is marked as consumed even when it is rejected, and a rejected
    /// call adds a diagnostic and yields no input: an entrypoint without forward
    /// support, or a spread or extra argument.
    #[allow(clippy::option_option)] // An accepted forward argument may be rejected.
    pub(crate) fn style_forward_argument(
        &mut self,
        expression: &Expression<'a>,
    ) -> Option<Option<StyleInput>> {
        let Expression::CallExpression(call) = expression.get_inner_expression() else {
            return None;
        };
        let spec = self.style_forward_call(call)?;
        Some(
            self.accept_forward(call, spec)
                .map(|value| StyleInput::Forward {
                    call: self.source_span(call.span),
                    value,
                    style_arrays: spec.forward == ForwardSupport::StyleArrays,
                }),
        )
    }

    /// Records a `style.forward(...)` second argument of a call that is not `style()`.
    ///
    /// The caller must not pass a `style()` call. The callee of the outer call is not
    /// checked, so the factory may be local, imported, or a parameter.
    pub(crate) fn collect_forward_call(&mut self, call: &CallExpression<'a>) {
        let Some(Argument::CallExpression(forward)) = call.arguments.get(1) else {
            return;
        };
        let Some(spec) = self.style_forward_call(forward) else {
            return;
        };
        if self.accept_forward(forward, spec).is_some() {
            self.forward_calls.push(ForwardCall {
                callee_span: self.source_span(forward.callee.span()),
                output: spec.output,
                style_arrays: spec.forward == ForwardSupport::StyleArrays,
            });
        }
    }

    /// Marks a recognized forward call as consumed and validates its entrypoint and
    /// arguments. Returns the span of the props argument, which is `None` for a call
    /// without arguments, or `None` after adding a diagnostic.
    #[allow(clippy::option_option)] // An accepted call may have no argument.
    fn accept_forward(
        &mut self,
        call: &CallExpression<'a>,
        spec: &'static FrameworkSpec,
    ) -> Option<Option<stillwind_source::Span>> {
        self.consumed_use_spans.insert(LocalSpan::from(call.span));
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.get_inner_expression().span()));
        self.consumed_macro_uses.push(self.source_span(call.span));
        if spec.forward == ForwardSupport::Unavailable {
            self.diagnostics.push(CompileError::new(
                CompileDiagnostic::UnavailableStyleForward {
                    entrypoint: spec.macro_import.into(),
                },
                self.source_span(call.span),
            ));
            return None;
        }
        match call.arguments.as_slice() {
            [] => Some(None),
            [argument] if !argument.is_spread() => Some(Some(self.source_span(argument.span()))),
            _ => {
                self.unconsumed_use(call.span, "style.forward(...)".into());
                None
            }
        }
    }
}
