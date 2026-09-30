//! Exact consumption checks for projection uses and ref variants. Receivers are
//! resolved through declaration bindings and checker facts before reporting a use.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::{GetSpan, Span};

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_semantic::AssetKind;

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Reports recognized calls absent from the exact set consumed by input parsers.
    /// Unresolved receivers register checker probes without producing a diagnostic.
    pub(crate) fn report_unconsumed_call(&mut self, call: &CallExpression<'a>) {
        let callee_span = call.callee.get_inner_expression().span();
        self.consumed_use_spans.insert(LocalSpan::from(callee_span));
        if self
            .consumed_use_spans
            .contains(&LocalSpan::from(call.span))
        {
            return;
        }
        if self.is_style_member(&call.callee, "anchorScope") {
            self.unconsumed_use(call.span, "style.anchorScope(...)".into());
            return;
        }
        if self.is_style_ref_call(call)
            || self.is_style_keyframes_call(call)
            || self.is_style_view_transition_call(call)
        {
            return;
        }
        if let Some((receiver, name, kind)) = macro_member(&call.callee)
            && self.use_receiver_is(receiver, kind)
        {
            self.unconsumed_use(call.span, format!("{name}(...)"));
            return;
        }
        if self.use_receiver_is(&call.callee, AssetKind::Keyframes) {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::KeyframesUseNotAClassInput,
                self.source_span(call.span),
            ));
        } else if self.use_receiver_is(&call.callee, AssetKind::Ref) {
            self.unconsumed_use(call.span, "ref variant".into());
        }
    }

    /// Checks property and tagged-template forms without accepting surrounding
    /// style-call containment as evidence that an input parser consumed them.
    pub(crate) fn report_unconsumed_expression(&mut self, expression: &Expression<'a>) {
        let span = expression.span();
        let consumed = self.consumed_use_spans.contains(&LocalSpan::from(span));
        if let Expression::TaggedTemplateExpression(tagged) = expression {
            let tag_span = tagged.tag.get_inner_expression().span();
            self.consumed_use_spans.insert(LocalSpan::from(tag_span));
            if consumed {
                return;
            }
            let (receiver, kind) = macro_member(&tagged.tag)
                .map_or((&tagged.tag, AssetKind::Ref), |(receiver, _, kind)| {
                    (receiver, kind)
                });
            if self.use_receiver_is(receiver, kind) {
                self.unconsumed_use(span, "macro variant tag".into());
            }
        } else if !consumed {
            if self.is_style_member(expression, "anchorScope") {
                self.unconsumed_use(span, "style.anchorScope".into());
            } else if let Some((receiver, name, kind)) = macro_member(expression)
                && self.use_receiver_is(receiver, kind)
            {
                self.unconsumed_use(span, name.into());
            }
        }
    }

    /// Records receiver probes and recognizes even non-finite checker facts for
    /// rejection; constructing a use still requires finite declaration candidates.
    fn use_receiver_is(&self, expression: &Expression<'a>, kind: AssetKind) -> bool {
        let expression = expression.get_inner_expression();
        if let Some(fact) = self.type_fact(expression.span()) {
            return fact.kind() == kind;
        }
        self.probing_use.set(true);
        let resolved = match kind {
            AssetKind::Ref => self.ref_references(expression).is_some(),
            AssetKind::Keyframes => self.keyframes_references(expression).is_some(),
            AssetKind::ViewTransition => self.view_transition_references(expression).is_some(),
            AssetKind::Style => false,
        };
        self.probing_use.set(false);
        resolved
    }

    fn unconsumed_use(&mut self, span: Span, syntax: String) {
        self.diagnostics.push(CompileError::new(
            CompileErrorKind::UnconsumedMacroUse { syntax },
            self.source_span(span),
        ));
    }
}

/// Returns recognized member syntax without treating its spelling as proof of an asset.
fn macro_member<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(&'e Expression<'a>, &'e str, AssetKind)> {
    let (receiver, name) = match expression.get_inner_expression() {
        Expression::StaticMemberExpression(member) => {
            (&member.object, member.property.name.as_str())
        }
        Expression::ComputedMemberExpression(member) => {
            let Expression::StringLiteral(name) = member.expression.get_inner_expression() else {
                return None;
            };
            (&member.object, name.value.as_str())
        }
        _ => return None,
    };
    let kind = match name {
        "asAnchor" | "asContainer" | "positionAnchor" | "at" => AssetKind::Ref,
        "for" => AssetKind::ViewTransition,
        _ => return None,
    };
    Some((receiver, name, kind))
}
