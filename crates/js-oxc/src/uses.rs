//! Exact consumption checks for projection uses and ref variants. Receivers are
//! resolved through declaration bindings and checker facts before reporting a use.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::{GetSpan, Span};

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_semantic::AssetKind;

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Checks for macro calls that no style-input parser has consumed.
    ///
    /// Call this after parsing supported style inputs. A recognized but unconsumed use
    /// adds a diagnostic. Unknown receivers add optional checker probes. Declaration
    /// calls are checked separately when validating declaration placement.
    ///
    /// The inner callee range is always marked as consumed. This prevents a second
    /// diagnostic for the method property when the visitor descends into the callee.
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
                CompileDiagnostic::KeyframesUseNotAClassInput,
                self.source_span(call.span),
            ));
        } else if self.use_receiver_is(&call.callee, AssetKind::Ref) {
            self.unconsumed_use(call.span, "ref variant".into());
        }
    }

    /// Checks whether a property access or tagged template is an unconsumed macro use.
    ///
    /// Call this after parsing supported style inputs and before visiting children.
    /// Recognized but unconsumed uses add diagnostics. Unknown receivers add optional
    /// checker probes.
    ///
    /// For a tagged template, the inner tag range is also marked as consumed so visiting
    /// its children does not produce a duplicate diagnostic.
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
            if self.is_style_member(expression.get_inner_expression(), "cva") {
                self.unconsumed_use(span, "style.cva".into());
            } else if self.is_style_member(expression, "anchorScope") {
                self.unconsumed_use(span, "style.anchorScope".into());
            } else if let Some((receiver, name, kind)) = macro_member(expression)
                && self.use_receiver_is(receiver, kind)
            {
                self.unconsumed_use(span, name.into());
            }
        }
    }

    /// Returns whether the receiver is an asset of `kind`.
    ///
    /// Checker facts can establish its kind even when they do not give a finite set of
    /// possible declarations. A true result therefore does not guarantee that the
    /// receiver can be lowered. Unknown receivers return false and may add optional
    /// checker probes.
    ///
    /// The caller must enter with `probing_use` set to false. During resolution, the
    /// function temporarily sets it to true and restores false before returning.
    fn use_receiver_is(&self, expression: &Expression<'a>, kind: AssetKind) -> bool {
        let expression = expression.get_inner_expression();
        if let Some(fact) = self.type_fact(expression.span()) {
            return matches!(fact, stillwind_semantic::CheckedValue::Asset(value) if value.kind() == kind);
        }
        self.probing_use.set(true);
        let resolved = match kind {
            AssetKind::Ref => self.ref_references(expression).is_ok(),
            AssetKind::Keyframes => self.keyframes_references(expression).is_ok(),
            AssetKind::ViewTransition => self.view_transition_references(expression).is_ok(),
            AssetKind::Style => false,
        };
        self.probing_use.set(false);
        resolved
    }

    /// Adds a diagnostic for the unconsumed macro `syntax` at `span`.
    ///
    /// The span must refer to the current AST buffer. It is converted with the
    /// extractor's source-span constructor before being added to the diagnostic.
    fn unconsumed_use(&mut self, span: Span, syntax: String) {
        self.diagnostics.push(CompileError::new(
            CompileDiagnostic::UnconsumedMacroUse { syntax },
            self.source_span(span),
        ));
    }
}

/// Returns the receiver, member name, and expected asset kind when the member name
/// matches known macro syntax.
///
/// Accepts static members and computed members with literal string or expression-free
/// template keys after unwrapping expressions. Other forms return `None`. The caller
/// must resolve the receiver identity and request any checker probes.
fn macro_member<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(&'e Expression<'a>, &'e str, AssetKind)> {
    let (receiver, name, _) = literal_member(expression)?;
    let kind = match name {
        "asAnchor" | "asContainer" | "positionAnchor" | "at" | "in" => AssetKind::Ref,
        "for" => AssetKind::ViewTransition,
        _ => return None,
    };
    Some((receiver, name, kind))
}

/// Returns a member's receiver, decoded name, and optional-access flag.
/// Static names, string literals, and expression-free templates are accepted after
/// unwrapping expressions. Dynamic keys and templates without cooked text return
/// `None`. Callers must check the optional-access flag when interpreting macro syntax.
pub(crate) fn literal_member<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(&'e Expression<'a>, &'e str, bool)> {
    match expression.get_inner_expression() {
        Expression::StaticMemberExpression(member) => Some((
            &member.object,
            member.property.name.as_str(),
            member.optional,
        )),
        Expression::ComputedMemberExpression(member) => {
            let name = match member.expression.get_inner_expression() {
                Expression::StringLiteral(name) => name.value.as_str(),
                Expression::TemplateLiteral(template)
                    if template.expressions.is_empty() && template.quasis.len() == 1 =>
                {
                    template.quasis[0].value.cooked.as_ref()?.as_str()
                }
                _ => return None,
            };
            Some((&member.object, name, member.optional))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use stillwind_diagnostics::CompileErrorCode;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_source::{FileId, ModuleId, SourceFile};

    use crate::{ParsedJavaScript, parse};

    fn parse_saved_use(expression: &str) -> (SourceFile, ParsedJavaScript) {
        let code = format!(
            "import {{style}} from 'stillwind/react';\n\
             // 日本語 🧭\n\
             const r = style.ref('r'); const s = style.ref('s');\n\
             const transition = style.viewTransition({{old: 'opacity-0'}});\n\
             const ordinary = {{in: (value) => value, at: () => 'plain'}};\n\
             const saved = {expression};"
        );
        let source = SourceFile::new(FileId(1), None, None, code).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("saved.ts"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        (source, parsed)
    }

    #[test]
    fn saved_computed_macro_calls_and_tags_report_unconsumed_use() {
        for expression in [
            "r['in'](s)",
            "r[`in`](s)",
            r"r[`i\u006e`](s)",
            "r()['in'](s)",
            "r('hover')[`in`](s)",
            "r`hover`[`in`](s)",
            "r?.[`in`](s)",
            "r[`in`]?.(s)",
            "r['at']()",
            "r[`at`]('width > 40rem')",
            "r[`at`]`width > 40rem`",
            "r?.[`at`]()",
            "r[`at`]?.()",
            "r[`asAnchor`]()",
            "r[`asContainer`]()",
            "r[`positionAnchor`]()",
            "transition[`for`]('card')",
        ] {
            let (source, parsed) = parse_saved_use(expression);
            assert!(
                parsed.diagnostics.iter().any(|error| {
                    error.code() == CompileErrorCode::UnconsumedMacroUse
                        && error
                            .labels
                            .iter()
                            .any(|label| expression.starts_with(source.slice(label.span).unwrap()))
                }),
                "{expression}: {:?}",
                parsed.diagnostics
            );
        }
    }

    #[test]
    fn saved_computed_macro_members_report_unconsumed_use() {
        for expression in [
            "r['in']",
            "r[`in`]",
            "r[`at`]",
            "r[`asAnchor`]",
            "transition[`for`]",
        ] {
            let (source, parsed) = parse_saved_use(expression);
            assert!(
                parsed.diagnostics.iter().any(|error| {
                    error.code() == CompileErrorCode::UnconsumedMacroUse
                        && error
                            .labels
                            .iter()
                            .any(|label| source.slice(label.span).unwrap() == expression)
                }),
                "{expression}: {:?}",
                parsed.diagnostics
            );
        }
    }

    #[test]
    fn computed_macro_names_on_ordinary_receivers_are_not_macro_uses() {
        for expression in ["ordinary[`in`](s)", "ordinary['at']()", "ordinary[`in`]"] {
            let (_, parsed) = parse_saved_use(expression);
            assert!(
                parsed.diagnostics.is_empty(),
                "{expression}: {:?}",
                parsed.diagnostics
            );
        }
    }
}
