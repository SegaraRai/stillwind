//! Oxc-backed JS/TS/JSX parsing and the first import-bound `style()` extractor.

use std::{collections::HashSet, path::Path};

use oxc::{
    allocator::Allocator,
    ast::ast::{
        Argument, CallExpression, Expression, ImportDeclaration, ImportDeclarationSpecifier,
        ModuleExportName,
    },
    ast_visit::{Visit, walk},
    parser::Parser,
    semantic::{Scoping, SemanticBuilder, SymbolId},
    span::{GetSpan, SourceType},
};
use shorth_ir::{ModuleIr, StyleCall, StyleInput};
use shorth_source::{Diagnostic, SourceFile, Span, Spanned};

#[derive(Debug)]
pub struct ParsedJavaScript {
    pub diagnostics: Vec<Diagnostic>,
    pub module: ModuleIr,
}

pub fn parse(source: &SourceFile) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(source.path())).unwrap_or_default();
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic::error("SHJS001", diagnostic.to_string(), source.full_span()))
        .collect();

    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(source, semantic.scoping());
    extractor.visit_program(&result.program);
    diagnostics.extend(extractor.diagnostics);

    ParsedJavaScript {
        diagnostics,
        module: ModuleIr {
            adapter: shorth_ir::AdapterKind::React,
            style_calls: extractor.style_calls,
        },
    }
}

struct MacroExtractor<'a> {
    source: &'a SourceFile,
    scoping: &'a Scoping,
    style_symbols: HashSet<SymbolId>,
    style_calls: Vec<StyleCall>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> MacroExtractor<'a> {
    fn new(source: &'a SourceFile, scoping: &'a Scoping) -> Self {
        Self {
            source,
            scoping,
            style_symbols: HashSet::new(),
            style_calls: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn source_span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    fn is_style_import(declaration: &ImportDeclaration<'a>) -> bool {
        declaration.source.value.as_str() == "shorth/react"
    }
}

impl<'a> Visit<'a> for MacroExtractor<'a> {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if Self::is_style_import(declaration)
            && let Some(specifiers) = &declaration.specifiers
        {
            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let ModuleExportName::IdentifierName(imported) = &specifier.imported else {
                    continue;
                };
                if imported.name.as_str() == "style"
                    && let Some(symbol_id) = specifier.local.symbol_id.get()
                {
                    self.style_symbols.insert(symbol_id);
                }
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let Expression::Identifier(callee) = &call.callee else {
            walk::walk_call_expression(self, call);
            return;
        };
        let Some(reference_id) = callee.reference_id.get() else {
            walk::walk_call_expression(self, call);
            return;
        };
        let Some(symbol_id) = self.scoping.get_reference(reference_id).symbol_id() else {
            walk::walk_call_expression(self, call);
            return;
        };
        if !self.style_symbols.contains(&symbol_id) {
            walk::walk_call_expression(self, call);
            return;
        }

        let mut inputs = Vec::new();
        for argument in &call.arguments {
            let Argument::StringLiteral(literal) = argument else {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS002",
                    "the first compiler slice accepts only literal utility strings in style()",
                    self.source_span(argument.span()),
                ));
                continue;
            };
            let literal_span = self.source_span(literal.span);
            let content_span = Span::new(
                literal_span.file,
                literal_span.start.0 + 1,
                literal_span.end.0.saturating_sub(1),
            );
            inputs.push(StyleInput::UtilityString(Spanned::new(
                literal.value.to_string(),
                content_span,
            )));
        }
        self.style_calls.push(StyleCall {
            span: self.source_span(call.span),
            inputs,
        });

        walk::walk_call_expression(self, call);
    }
}

#[cfg(test)]
mod tests {
    use shorth_source::FileId;

    use super::*;

    #[test]
    fn accepts_typescript_jsx() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            "const Button = ({ loading }: { loading: boolean }) => <button>{loading}</button>;",
        );

        assert!(parse(&source).diagnostics.is_empty());
    }

    #[test]
    fn extracts_only_an_import_bound_style_call_with_literal_spans() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            r#"import { style as s } from "shorth/react";
const props = s("px-4 text-center", "hover:bg-accent");
function local(s: (value: string) => unknown) { s("not-a-style-call"); }"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.module.style_calls.len(), 1);
        let StyleInput::UtilityString(first) = &parsed.module.style_calls[0].inputs[0];
        assert_eq!(first.value, "px-4 text-center");
        assert_eq!(source.slice(first.span).unwrap(), "px-4 text-center");
    }
}
