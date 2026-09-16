//! The main AST visit that turns a program into [`stillwind_ir::ModuleIr`] and a
//! summary.
//!
//! The visit records four kinds of fact: module structure (imports, re-exports,
//! specifier literals, exported bindings), declarations of assets (`style.ref()`,
//! `style.scope()`, animations, view transitions), `style()` call sites, and the JSX
//! ancestry of each visited element so ref/scope validation can check containment
//! later.
//!
//! Recognition always goes through resolved symbols. The one place spelling is used is
//! [`ImportBinding`], because an import declaration is the only construct that can
//! associate a name with a module before any symbol exists.

use oxc::ast::ast::{
    Argument, BindingPattern, CallExpression, Declaration, ExportAllDeclaration, ExportDeclaration,
    ExportFromDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, JSXElement, ModuleExportName, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{StyleCall, from_macro_import};
use stillwind_semantic::{
    AssetKind, ExportedValue, ImportBinding, ModuleSpecifier, ReExport, RefAlternative,
    SymbolicValue,
};
use stillwind_source::Spanned;

use crate::state::MacroExtractor;

impl<'a> Visit<'a> for MacroExtractor<'a> {
    /// Tracks the JSX ancestor path and gives each element a stable index.
    ///
    /// Path indices are assigned in visit order, so a sibling comparison later uses the
    /// same numbering the path was recorded with.
    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        let element_id = self.next_jsx_element;
        self.next_jsx_element += 1;
        self.jsx_path.push(element_id);
        walk::walk_jsx_element(self, element);
        self.jsx_path.pop();
    }

    /// Records module specifiers, macro imports and ordinary import bindings.
    ///
    /// A specifier literal is recorded for every import, because the host needs the
    /// exact span to resolve shadow modules; only value imports of a macro entry point
    /// contribute style symbols, and only non-macro imports contribute
    /// [`ImportBinding`]s.
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        self.module_specifiers.push(ModuleSpecifier {
            source: declaration.source.value.to_string(),
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
                            CompileErrorKind::MixedFrameworks,
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
                        specifier.local.name.to_string(),
                        Self::module_export_name(&specifier.imported),
                    ),
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        (specifier.local.name.to_string(), "default".to_owned())
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (specifier.local.name.to_string(), "*".to_owned())
                    }
                };
                self.imports.push(ImportBinding {
                    local: local.clone(),
                    source: declaration.source.value.to_string(),
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
                        .push((symbol, identifier.name.to_string()));
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
            source: declaration.source.value.to_string(),
            start: declaration.source.span.start,
            end: declaration.source.span.end,
        });
        for specifier in &declaration.specifiers {
            self.re_exports.push(ReExport {
                exported: Self::module_export_name(&specifier.exported),
                source: declaration.source.value.to_string(),
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
            source: declaration.source.value.to_string(),
            start: declaration.source.span.start,
            end: declaration.source.span.end,
        });
        self.re_exports.push(ReExport {
            exported: declaration
                .exported
                .as_ref()
                .map_or_else(|| "*".to_owned(), Self::module_export_name),
            source: declaration.source.value.to_string(),
            imported: "*".to_owned(),
        });
        walk::walk_export_all_declaration(self, declaration);
    }

    /// Collects one `style(...)` call site.
    ///
    /// Only calls whose callee resolves to the imported macro are collected. A call
    /// nested inside an already-collected one is reported as `SHCOMP001` instead of
    /// being lowered, and spread arguments are errors.
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

        let span = self.source_span(call.span);
        if self
            .style_calls
            .iter()
            .any(|parent| parent.span.start <= span.start && parent.span.end >= span.end)
        {
            self.diagnostics
                .push(CompileError::new(CompileErrorKind::NestedStyleCall, span));
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

    /// Recognizes the four asset producers in one `const x = style.<member>(...)`.
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
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_ref_definition(name, call) {
                self.ref_symbols.insert(symbol_id, definition.clone());
                let scopes = definition.scope.as_ref().map(|scope| {
                    SymbolicValue::exact(self.identity(AssetKind::Scope, &scope.binding))
                });
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Refs(vec![RefAlternative {
                        reference: self.identity(AssetKind::Ref, &definition.binding),
                        permitted_scopes: scopes,
                    }]),
                );
                self.ref_definitions.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_scope_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_scope_definition(name, call) {
                self.scope_symbols.insert(symbol_id, definition.clone());
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Scopes(SymbolicValue::exact(
                        self.identity(AssetKind::Scope, &definition.binding),
                    )),
                );
                self.scope_definitions.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_animation_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_animation_definition(name.clone(), call) {
                self.animation_symbols.insert(symbol_id, name);
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Animations(SymbolicValue::exact(
                        self.identity(AssetKind::Animation, &definition.binding),
                    )),
                );
                self.animation_declarations.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_view_transition_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_view_transition_definition(name.clone(), call) {
                self.view_transition_symbols.insert(symbol_id, name);
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::ViewTransitions(SymbolicValue::exact(
                        self.identity(AssetKind::ViewTransition, &definition.binding),
                    )),
                );
                self.view_transition_declarations.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && let Some(application) = self.collect_view_transition_key_application(call)
        {
            // The binding denotes a transition that already carries its key, so a later use
            // reads the name from the value instead of deriving it from a use-site key.
            if let Some(symbol_id) = identifier.symbol_id.get()
                && let Ok(value) = SymbolicValue::finite(
                    AssetKind::ViewTransition,
                    application
                        .transitions
                        .iter()
                        .map(|transition| transition.identity.clone()),
                )
            {
                self.view_transition_key_symbols.insert(
                    symbol_id,
                    application
                        .transitions
                        .iter()
                        .map(|transition| transition.identity.clone())
                        .collect(),
                );
                self.local_values
                    .insert(symbol_id, ExportedValue::ViewTransitionKeys(value));
            }
            self.view_transition_key_applications.push(application);
        }
        walk::walk_variable_declarator(self, declarator);
    }
}
