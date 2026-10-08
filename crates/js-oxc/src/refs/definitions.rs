//! Static ref declarations and target conditions written as ref calls, tags, or scopes.
//!
//! Declarations accept an optional literal label. Ref conditions keep their element
//! prefix separate from the independent shared scope conditions supplied by `in()`.
//! Container queries remain conditions on the final styled target.

use oxc::ast::ast::{CallExpression, Expression};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{RefCondition, RefDefinition, StyleInput, StyleVariant};
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;
use crate::uses::literal_member;

/// Static text from a call or tag, or a declaration used as a presence operand.
enum RefVariantText<'e, 'a> {
    Bare(&'e Expression<'a>),
    Prefix {
        reference: &'e Expression<'a>,
        prefix: Spanned<String>,
    },
}

impl<'a> MacroExtractor<'a> {
    /// Extracts a declaration with no arguments or one literal label. Reports invalid
    /// arguments and returns `None` without registering a declaration.
    pub(crate) fn collect_ref_definition(
        &mut self,
        binding: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<RefDefinition> {
        let label = match call.arguments.as_slice() {
            [] => None,
            [argument] => {
                let Some(Expression::StringLiteral(label)) = argument.as_expression() else {
                    self.ref_syntax_error(CompileDiagnostic::InvalidRefArgument, argument.span());
                    return None;
                };
                Some(self.string_literal(label))
            }
            _ => {
                self.ref_syntax_error(CompileDiagnostic::InvalidRefArguments, call.span);
                return None;
            }
        };
        Some(RefDefinition {
            module_id: self.module_id.clone(),
            binding,
            label,
            call_span: self.source_span(call.span),
        })
    }

    /// Consumes a direct variant expression inside `style()`. Parses static ref calls
    /// and tags, optional outer scope calls, or a raw ref's container query. Records
    /// identities and checker probes. Invalid syntax returns `None` after adding a
    /// diagnostic and consuming its nested macro uses.
    pub(crate) fn ref_variant(&mut self, expression: &Expression<'a>) -> Option<StyleVariant> {
        self.consume_ref_expression(expression);
        let mut receiver = expression.get_inner_expression();
        let mut scopes = Vec::new();
        while let Expression::CallExpression(call) = receiver {
            let Some((object, name, optional)) = literal_member(&call.callee) else {
                break;
            };
            if name != "in" {
                break;
            }
            if call.optional || optional || call.arguments.len() != 1 {
                self.ref_syntax_error(CompileDiagnostic::InvalidRefScopeArguments, call.span);
                return None;
            }
            let Some(scope) = call.arguments[0].as_expression() else {
                self.ref_syntax_error(CompileDiagnostic::InvalidRefScopeArguments, call.span);
                return None;
            };
            scopes.push(self.ref_condition(scope, true));
            receiver = object.get_inner_expression();
        }
        scopes.reverse();
        if let RefVariantText::Prefix {
            reference,
            prefix: query,
        } = self.ref_variant_text(receiver)?
            && let Some((object, name, optional)) = literal_member(reference)
            && name == "at"
        {
            if !scopes.is_empty() || optional {
                self.ref_syntax_error(
                    CompileDiagnostic::InvalidRefVariantReceiver,
                    expression.span(),
                );
                return None;
            }
            return self.ref_container_variant(object, query);
        }
        // Resolve every operand before returning on missing identities so the scan
        // records all checker probes needed by finalization.
        let condition = self.ref_condition(receiver, !scopes.is_empty());
        let scopes = scopes.into_iter().collect::<Option<Vec<_>>>()?;
        let condition = condition?;
        Some(StyleVariant::Ref { condition, scopes })
    }

    /// Resolves a raw ref receiver into a named container query variant.
    /// `query` contains static text and its content span. Returns `None` with a
    /// diagnostic when the receiver cannot resolve to finite ref identities.
    fn ref_container_variant(
        &mut self,
        receiver: &Expression<'a>,
        query: Spanned<String>,
    ) -> Option<StyleVariant> {
        let Some(refs) = self.ref_references(receiver) else {
            self.ref_syntax_error(
                CompileDiagnostic::InvalidRefVariantReceiver,
                receiver.span(),
            );
            return None;
        };
        Some(StyleVariant::Container { refs, query })
    }

    /// Reads a static call or tag without resolving its receiver. Identifies bare
    /// expressions separately. Reports optional calls, interpolated tags and
    /// nonliteral arguments as invalid static variant text.
    fn ref_variant_text<'e>(
        &mut self,
        expression: &'e Expression<'a>,
    ) -> Option<RefVariantText<'e, 'a>> {
        match expression.get_inner_expression() {
            Expression::TaggedTemplateExpression(tagged) => {
                if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
                    self.ref_syntax_error(
                        CompileDiagnostic::NonStaticRefVariant,
                        tagged.quasi.span,
                    );
                    return None;
                }
                let cooked = tagged.quasi.quasis[0].value.cooked.as_ref()?;
                let span = self.source_span(tagged.quasi.span);
                Some(RefVariantText::Prefix {
                    reference: &tagged.tag,
                    prefix: Spanned::new(
                        cooked.to_string(),
                        Span::new(span.file, span.start.0 + 1, span.end.0.saturating_sub(1)),
                    ),
                })
            }
            Expression::CallExpression(call) if !call.optional => {
                let text = match call.arguments.as_slice() {
                    [] => Spanned::new(String::new(), self.source_span(call.span)),
                    [argument] => {
                        let Some(Expression::StringLiteral(value)) = argument.as_expression()
                        else {
                            self.ref_syntax_error(
                                CompileDiagnostic::NonStaticRefVariant,
                                call.span,
                            );
                            return None;
                        };
                        self.string_literal(value)
                    }
                    _ => {
                        self.ref_syntax_error(CompileDiagnostic::NonStaticRefVariant, call.span);
                        return None;
                    }
                };
                Some(RefVariantText::Prefix {
                    reference: &call.callee,
                    prefix: text,
                })
            }
            Expression::CallExpression(call) => {
                self.ref_syntax_error(CompileDiagnostic::NonStaticRefVariant, call.span);
                None
            }
            _ => Some(RefVariantText::Bare(expression)),
        }
    }

    /// Resolves an unscoped element condition. Bare refs are accepted only for scope
    /// operands and the presence shorthand before an outer `in()` call. Container
    /// queries and nested scope operands are rejected before identity resolution.
    fn ref_condition(
        &mut self,
        expression: &Expression<'a>,
        allow_presence: bool,
    ) -> Option<RefCondition> {
        self.consume_ref_expression(expression);
        let expression = expression.get_inner_expression();
        if let Expression::CallExpression(call) = expression
            && let Some((_, name, _)) = literal_member(&call.callee)
            && name == "in"
        {
            self.ref_syntax_error(CompileDiagnostic::NestedRefScope, expression.span());
            return None;
        }
        let (reference, mut prefix) = match self.ref_variant_text(expression)? {
            RefVariantText::Prefix { reference, prefix } => (reference, prefix),
            RefVariantText::Bare(expression) if allow_presence => (
                expression,
                Spanned::new(String::new(), self.source_span(expression.span())),
            ),
            RefVariantText::Bare(_) => {
                self.ref_syntax_error(CompileDiagnostic::UnidentifiedRefVariant, expression.span());
                return None;
            }
        };
        if let Some((_, name, _)) = literal_member(reference)
            && matches!(name, "in" | "at")
        {
            let diagnostic = if name == "in" {
                CompileDiagnostic::NestedRefScope
            } else {
                CompileDiagnostic::InvalidRefVariantReceiver
            };
            self.ref_syntax_error(diagnostic, expression.span());
            return None;
        }
        if matches!(
            reference.get_inner_expression(),
            Expression::CallExpression(_) | Expression::TaggedTemplateExpression(_)
        ) {
            self.ref_syntax_error(
                CompileDiagnostic::InvalidRefVariantReceiver,
                reference.span(),
            );
            return None;
        }
        let Some(refs) = self.ref_references(reference) else {
            self.ref_syntax_error(CompileDiagnostic::UnidentifiedRefVariant, reference.span());
            return None;
        };
        if !prefix.value.is_empty() && !prefix.value.ends_with(':') {
            prefix.value.push(':');
        }
        Some(RefCondition { refs, prefix })
    }

    /// Marks the call/tag chain and its scope operands as consumed. This prevents the
    /// AST visitor from reporting inner calls again after validating the whole chain.
    fn consume_ref_expression(&mut self, expression: &Expression<'a>) {
        let expression = expression.get_inner_expression();
        self.consumed_use_spans
            .insert(LocalSpan::from(expression.span()));
        match expression {
            Expression::CallExpression(call) => {
                self.consume_ref_expression(&call.callee);
                for argument in &call.arguments {
                    if let Some(argument) = argument.as_expression() {
                        self.consume_ref_expression(argument);
                    }
                }
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.consume_ref_expression(&tagged.tag);
            }
            Expression::StaticMemberExpression(member) => {
                self.consume_ref_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.consume_ref_expression(&member.object);
            }
            _ => {}
        }
    }

    /// Adds a fatal ref syntax diagnostic at a span in the current analysis buffer.
    fn ref_syntax_error(&mut self, diagnostic: CompileDiagnostic, span: oxc::span::Span) {
        self.diagnostics
            .push(CompileError::new(diagnostic, self.source_span(span)));
    }

    /// Recognizes a ref used as a `style()` argument.
    ///
    /// `expression` must come from a supported input position in the current AST.
    /// A finite ref value appends one attachment to `inputs` and returns `true`.
    /// One candidate becomes a static attachment; multiple or zero candidates retain
    /// the runtime receiver expression. Original source coordinates identify the use.
    /// Unrelated expressions return `false` without appending an attachment. Receiver
    /// resolution may still record checker probes and consumed macro expressions.
    pub(crate) fn collect_dynamic_ref_attachment(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) -> bool {
        if let Some(refs) = self.ref_references(expression) {
            if refs.len() == 1 {
                inputs.push(StyleInput::RefAttachment {
                    definition: refs[0].clone(),
                    value: self.source_span(expression.span()),
                });
            } else {
                inputs.push(StyleInput::DynamicRefAttachment {
                    value: self.source_span(expression.span()),
                    refs,
                });
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use stillwind_diagnostics::CompileErrorCode;
    use stillwind_ir::{
        ClassExpression, GroupEntry, RuleExpression, ScriptLanguage, ScriptModuleKind, StyleInput,
        StyleVariant,
    };
    use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact};
    use stillwind_source::{FileId, ModuleId, SourceFile};

    use crate::{ParsedJavaScript, parse};

    fn parse_variant_input(input: &str) -> (SourceFile, ParsedJavaScript) {
        let code = format!(
            "import {{style}} from 'stillwind/react';\n\
             // 日本語 🧭\n\
             const target = style.ref('target'); const inner = style.ref('inner');\n\
             const outer = style.ref('outer'); const alias = target;\n\
             declare const choose: boolean;\n\
             export const result = style({input});"
        );
        let source = SourceFile::new(FileId(1), None, None, code).unwrap();
        let target_start = source.text().find("target =").unwrap();
        let target = IdentityId::new(
            AssetKind::Ref,
            "variants.ts".into(),
            u32::try_from(target_start).unwrap(),
            u32::try_from(target_start + "target".len()).unwrap(),
            "target",
        );
        let alias_facts = source
            .text()
            .match_indices("alias")
            .map(|(start, name)| TypeFact {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + name.len()).unwrap(),
                value: ExportedValue::Refs(SymbolicValue::exact(target.clone())),
            })
            .collect::<Vec<_>>();
        let parsed = parse(
            &source,
            &ModuleId::from("variants.ts"),
            &alias_facts,
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        (source, parsed)
    }

    fn extracted_variant(parsed: &ParsedJavaScript) -> &StyleVariant {
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let StyleInput::Class(expression) = &parsed.module.style_calls[0].inputs[0] else {
            panic!("expected class input");
        };
        let mut expression = expression;
        if let ClassExpression::Group { entries, .. } = expression {
            let GroupEntry::Class(inner) = &entries[0] else {
                panic!("expected class entry");
            };
            expression = inner;
        }
        let ClassExpression::Rule(rule) = expression else {
            panic!("expected rule expression");
        };
        let RuleExpression::Variant { variant, .. } = rule.as_ref() else {
            panic!("expected variant");
        };
        variant
    }

    #[test]
    fn computed_scope_members_keep_conditions_and_text_spans() {
        for expression in [
            "target('hover')['in'](inner('focus'))[\"in\"](outer)",
            "target`hover`[`in`](inner`focus`)[`in`](outer)",
            r"alias('hover')[`i\u006e`](inner('focus'))['in'](outer)",
        ] {
            for input in [
                format!("{{[{expression}]: 'flex'}}"),
                format!("`${{{expression}}}:flex`"),
            ] {
                let (source, parsed) = parse_variant_input(&input);
                let StyleVariant::Ref { condition, scopes } = extracted_variant(&parsed) else {
                    panic!("{expression}: expected ref variant");
                };
                assert_eq!(condition.refs[0].binding.value, "target");
                assert_eq!(condition.prefix.value, "hover:");
                assert_eq!(source.slice(condition.prefix.span).unwrap(), "hover");
                assert_eq!(scopes.len(), 2);
                assert_eq!(scopes[0].refs[0].binding.value, "inner");
                assert_eq!(scopes[0].prefix.value, "focus:");
                assert_eq!(source.slice(scopes[0].prefix.span).unwrap(), "focus");
                assert_eq!(scopes[1].refs[0].binding.value, "outer");
                assert_eq!(scopes[1].prefix.value, "");
                assert_eq!(source.slice(scopes[1].prefix.span).unwrap(), "outer");
            }
        }
    }

    #[test]
    fn computed_presence_scopes_preserve_finite_candidates() {
        let (_, parsed) = parse_variant_input(
            "{[(choose ? alias : outer)[`in`](choose ? inner : outer)]: 'flex'}",
        );
        let StyleVariant::Ref { condition, scopes } = extracted_variant(&parsed) else {
            panic!("expected ref variant");
        };
        assert_eq!(
            condition
                .refs
                .iter()
                .map(|reference| reference.binding.value.as_str())
                .collect::<Vec<_>>(),
            ["target", "outer"]
        );
        assert_eq!(condition.prefix.value, "");
        assert_eq!(scopes.len(), 1);
        assert_eq!(
            scopes[0]
                .refs
                .iter()
                .map(|reference| reference.binding.value.as_str())
                .collect::<Vec<_>>(),
            ["inner", "outer"]
        );
    }

    #[test]
    fn computed_container_members_keep_query_text_and_spans() {
        for expression in [
            "alias['at']('width > 40rem')",
            "target[\"at\"]`width > 40rem`",
            "target[`at`]('width > 40rem')",
            r"target[`a\u0074`]`width > 40rem`",
        ] {
            let (source, parsed) = parse_variant_input(&format!("{{[{expression}]: 'flex'}}"));
            let StyleVariant::Container { refs, query } = extracted_variant(&parsed) else {
                panic!("{expression}: expected container variant");
            };
            assert_eq!(refs[0].binding.value, "target");
            assert_eq!(query.value, "width > 40rem");
            assert_eq!(source.slice(query.span).unwrap(), "width > 40rem");
        }
        for expression in ["target['at']()", "target[`at`]('')", "target[`at`]``"] {
            let (_, parsed) = parse_variant_input(&format!("{{[{expression}]: 'flex'}}"));
            let StyleVariant::Container { query, .. } = extracted_variant(&parsed) else {
                panic!("{expression}: expected container variant");
            };
            assert_eq!(query.value, "");
        }
    }

    #[test]
    fn computed_members_reject_scoped_operands_and_completed_receivers() {
        for (expression, expected) in [
            (
                "target['in'](inner[`in`](outer))",
                CompileErrorCode::NestedRefScope,
            ),
            (
                "target[`in`](inner['at']())",
                CompileErrorCode::InvalidRefVariantReceiver,
            ),
            (
                "target['at']()[`in`](inner)",
                CompileErrorCode::InvalidRefVariantReceiver,
            ),
            (
                "target()['at']()",
                CompileErrorCode::InvalidRefVariantReceiver,
            ),
            (
                "target[`in`](inner)['at']()",
                CompileErrorCode::InvalidRefVariantReceiver,
            ),
            (
                "target[`in`](inner)('hover')",
                CompileErrorCode::InvalidRefVariantReceiver,
            ),
            ("target['in']()", CompileErrorCode::InvalidRefScopeArguments),
            (
                "target[`in`](...inner)",
                CompileErrorCode::InvalidRefScopeArguments,
            ),
        ] {
            let (_, parsed) = parse_variant_input(&format!("{{[{expression}]: 'flex'}}"));
            assert!(
                parsed
                    .diagnostics
                    .iter()
                    .any(|error| error.code() == expected),
                "{expression}: {:?}",
                parsed.diagnostics
            );
        }
    }

    #[test]
    fn computed_members_reject_optional_and_dynamic_syntax() {
        for expression in [
            "target?.['in'](inner)",
            "target['in']?.(inner)",
            "target?.[`at`]()",
            "target[`at`]?.()",
            "target()['in'](inner?.())",
            "target['in'](inner?.['in'](outer))",
            "target[`i${'n'}`](inner)",
            "target[`a${'t'}`]()",
            "target[choose ? 'in' : 'at'](inner)",
        ] {
            let (_, parsed) = parse_variant_input(&format!("{{[{expression}]: 'flex'}}"));
            assert!(
                !parsed.diagnostics.is_empty(),
                "{expression}: accepted unsupported syntax"
            );
        }
    }

    #[test]
    fn unresolved_imported_scopes_record_every_operand_probe() {
        for expression in [
            "target('hover').in(inner).in(outer)",
            "target`hover:`.in(inner`focus:`).in(outer('active'))",
            "target.in(inner).in(outer)",
            "target('hover')['in'](inner)[`in`](outer)",
            "target`hover:`[`in`](inner`focus:`)['in'](outer('active'))",
            "target[`in`](inner)[\"in\"](outer)",
            "(choose ? first : second)('hover').in(choose ? inner : outer)",
        ] {
            let code = format!(
                "import {{style}} from 'stillwind/react';\n\
                 import {{target, first, second, inner, outer}} from './refs';\n\
                 declare const choose: boolean;\n\
                 export const result = style({{[{expression}]: 'flex'}});"
            );
            let source = SourceFile::new(FileId(1), None, None, code).unwrap();
            let parsed = parse(
                &source,
                &ModuleId::from("consumer.ts"),
                &[],
                &BTreeMap::new(),
                ScriptLanguage::Ts,
                ScriptModuleKind::Module,
            );
            let operands: &[&str] = if expression.contains("choose ?") {
                &["first", "second", "inner", "outer"]
            } else {
                &["target", "inner", "outer"]
            };
            for operand in operands {
                let start = source.text().rfind(operand).unwrap();
                assert!(
                    parsed.type_probes.iter().any(|span| {
                        span.start.0 as usize == start
                            && span.end.0 as usize == start + operand.len()
                    }),
                    "{expression}: missing {operand} probe: {:?}",
                    parsed.type_probes
                );
            }
        }
    }
}
