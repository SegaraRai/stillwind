//! The main AST visit that turns a program into [`stillwind_ir::ModuleIr`] and a
//! summary.
//!
//! The visit records module structure (imports, re-exports, specifier literals,
//! exported bindings), declarations of assets (`style.ref()`, keyframes, view
//! transitions), `style()` call sites, and `style.media()` and `style.css()` calls.
//!
//! Recognition always goes through resolved symbols. The one place spelling is used is
//! [`ImportBinding`], because an import declaration is the only construct that can
//! associate a name with a module before any symbol exists.

use lean_string::ToLeanStr;
use oxc::ast::ast::{
    Argument, BindingPattern, CallExpression, Declaration, ExportAllDeclaration, ExportDeclaration,
    ExportFromDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, ModuleExportName, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{StyleCall, from_macro_import};
use stillwind_semantic::{
    AssetKind, ExportedValue, ImportBinding, ModuleSpecifier, ReExport, SymbolicValue,
};
use stillwind_source::Spanned;

use crate::local_span::LocalSpan;
use crate::state::MacroExtractor;

impl<'a> Visit<'a> for MacroExtractor<'a> {
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if self.input_is_omitted(expression.span()) {
            return;
        }
        self.report_unconsumed_expression(expression);
        walk::walk_expression(self, expression);
    }

    /// Records module specifiers, macro imports and ordinary import bindings.
    ///
    /// A specifier literal is recorded for every import, because the host needs the
    /// exact span to resolve shadow modules; only value imports of a macro entry point
    /// contribute style symbols, and only non-macro imports contribute
    /// [`ImportBinding`]s.
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        self.module_specifiers.push(ModuleSpecifier {
            source: declaration.source.value.to_lean_str(),
            start: declaration.source.span.start,
            end: declaration.source.span.end,
        });
        if Self::is_style_import(declaration)
            && !declaration.import_kind.is_type()
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
                    && !specifier.import_kind.is_type()
                    && let Some(symbol_id) = specifier.local.symbol_id.get()
                {
                    self.style_symbols.insert(symbol_id);
                    let Some(spec) = from_macro_import(declaration.source.value.as_str()) else {
                        continue;
                    };
                    let adapter = spec.id;
                    if self.adapter.is_some_and(|previous| previous != adapter) {
                        self.diagnostics.push(CompileError::new(
                            CompileDiagnostic::MixedFrameworks,
                            self.source_span(declaration.span),
                        ));
                    }
                    self.adapter = Some(adapter);
                }
            }
        }
        if !Self::is_style_import(declaration)
            && let Some(specifiers) = &declaration.specifiers
        {
            for specifier in specifiers {
                let (local, imported) = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => (
                        specifier.local.name.to_lean_str(),
                        Self::module_export_name(&specifier.imported),
                    ),
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        (specifier.local.name.to_lean_str(), "default".to_lean_str())
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (specifier.local.name.to_lean_str(), "*".to_lean_str())
                    }
                };
                self.imports.push(ImportBinding {
                    local: local.clone(),
                    source: declaration.source.value.to_lean_str(),
                    imported,
                });
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    /// Records exported variable names so their imports can be sliced later.
    ///
    /// Only direct `export const x = ...` bindings are recorded; destructuring patterns
    /// are left to the host checker.
    fn visit_export_declaration(&mut self, declaration: &ExportDeclaration<'a>) {
        if let Declaration::VariableDeclaration(variables) = &declaration.declaration {
            for declarator in &variables.declarations {
                if let BindingPattern::BindingIdentifier(identifier) = &declarator.id
                    && let Some(symbol) = identifier.symbol_id.get()
                {
                    self.export_symbols
                        .push((symbol, identifier.name.to_lean_str()));
                }
            }
        }
        walk::walk_export_declaration(self, declaration);
    }

    /// Records `export { local as name }` as an exported binding.
    ///
    /// The local side must be an identifier reference so the exported name can be
    /// attached to a symbol; string-literal locals are skipped.
    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        for specifier in &declaration.specifiers {
            let ModuleExportName::IdentifierReference(local) = &specifier.local else {
                continue;
            };
            let Some(reference) = local.reference_id.get() else {
                continue;
            };
            let Some(symbol) = self.scoping.get_reference(reference).symbol_id() else {
                continue;
            };
            self.export_symbols
                .push((symbol, Self::module_export_name(&specifier.exported)));
        }
        walk::walk_export_named_declaration(self, declaration);
    }

    /// Records `export ... from` specifiers, literals and re-export edges.
    ///
    /// The re-export edge keeps both spellings, so a consumer can follow the chain
    /// without re-parsing this module.
    fn visit_export_from_declaration(&mut self, declaration: &ExportFromDeclaration<'a>) {
        self.module_specifiers.push(ModuleSpecifier {
            source: declaration.source.value.to_lean_str(),
            start: declaration.source.span.start,
            end: declaration.source.span.end,
        });
        for specifier in &declaration.specifiers {
            self.re_exports.push(ReExport {
                exported: Self::module_export_name(&specifier.exported),
                source: declaration.source.value.to_lean_str(),
                imported: Self::module_export_name(&specifier.local),
            });
        }
        walk::walk_export_from_declaration(self, declaration);
    }

    /// Records `export * from` as a re-export with `*` on both sides.
    ///
    /// A named `export * as ns from` keeps its exported name, so the star form is
    /// distinguishable from a genuine `*` re-export.
    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        self.module_specifiers.push(ModuleSpecifier {
            source: declaration.source.value.to_lean_str(),
            start: declaration.source.span.start,
            end: declaration.source.span.end,
        });
        self.re_exports.push(ReExport {
            exported: declaration
                .exported
                .as_ref()
                .map_or_else(|| "*".to_lean_str(), Self::module_export_name),
            source: declaration.source.value.to_lean_str(),
            imported: "*".to_lean_str(),
        });
        walk::walk_export_all_declaration(self, declaration);
    }

    /// Collects one `style(...)` call site.
    ///
    /// Only calls whose callee resolves to the imported macro are collected. A call
    /// nested inside an already-collected one produces
    /// [`LintDiagnostic::NestedStyleCall`] and is omitted. Unsupported spread arguments
    /// are also omitted with lint findings.
    /// Every other call is checked for
    /// a macro use the lowering cannot consume.
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.is_style_member(call.callee.get_inner_expression(), "cva") {
            self.collect_cva(call);
            walk::walk_call_expression(self, call);
            return;
        }
        if self.is_style_member(call.callee.get_inner_expression(), "media") {
            self.collect_media(call);
            walk::walk_call_expression(self, call);
            return;
        }
        if self.is_style_member(call.callee.get_inner_expression(), "css") {
            self.collect_css(call);
            walk::walk_call_expression(self, call);
            return;
        }
        if !self.is_style_call(call) {
            self.report_unconsumed_call(call);
            walk::walk_call_expression(self, call);
            return;
        }

        let span = self.source_span(call.span);
        if self
            .style_calls
            .iter()
            .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
            || self
                .cva_calls
                .iter()
                .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
        {
            if !self
                .consumed_use_spans
                .contains(&LocalSpan::from(call.span))
            {
                self.lint_findings
                    .push(LintFinding::new(LintDiagnostic::NestedStyleCall, span));
            }
            return;
        }

        let mut inputs = Vec::new();
        for argument in &call.arguments {
            if let Argument::SpreadElement(spread) = argument {
                self.collect_style_spread(&spread.argument, &mut inputs);
            } else if let Some(expression) = argument.as_expression() {
                self.collect_style_expression(expression, &mut inputs);
            }
        }
        self.style_calls.push(StyleCall {
            span: self.source_span(call.span),
            inputs,
        });

        walk::walk_call_expression(self, call);
    }

    /// Recognizes the three asset producers in one `const x = style.<member>(...)`.
    ///
    /// Each producer records its definition, its symbol, and the symbolic value the
    /// binding denotes, so later uses resolve without re-checking the call.
    #[allow(clippy::too_many_lines)] // One declarator has several declaration forms.
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_ref_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_lean_str(),
                self.source_span(identifier.span),
            );
            let definition = self.collect_ref_definition(name, call);
            self.ref_symbols.insert(symbol_id, definition.clone());
            self.local_values.insert(
                symbol_id,
                ExportedValue::Refs(SymbolicValue::exact(
                    self.identity(AssetKind::Ref, &definition.binding),
                )),
            );
            self.ref_definitions.push(definition);
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_keyframes_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_lean_str(),
                self.source_span(identifier.span),
            );
            let definition = self.collect_keyframes_definition(name.clone(), call);
            self.keyframes_symbols.insert(symbol_id, name);
            self.local_values.insert(
                symbol_id,
                ExportedValue::Keyframes(SymbolicValue::exact(
                    self.identity(AssetKind::Keyframes, &definition.binding),
                )),
            );
            self.keyframes_declarations.push(definition);
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_view_transition_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_lean_str(),
                self.source_span(identifier.span),
            );
            let definition = self.collect_view_transition_definition(name.clone(), call);
            self.view_transition_symbols.insert(symbol_id, name);
            self.local_values.insert(
                symbol_id,
                ExportedValue::ViewTransitions(SymbolicValue::exact(
                    self.identity(AssetKind::ViewTransition, &definition.binding),
                )),
            );
            self.view_transition_declarations.push(definition);
        }
        walk::walk_variable_declarator(self, declarator);
    }
}
