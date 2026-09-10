use oxc::ast::ast::{
    Argument, BindingPattern, CallExpression, Declaration, ExportAllDeclaration, ExportDeclaration,
    ExportFromDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, JSXElement, ModuleExportName, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};

use stillwind_ir::StyleCall;
use stillwind_ir::framework::from_macro_import;
use stillwind_semantic::{
    AssetKind, ExportedValue, ImportBinding, ModuleSpecifier, ReExport, RefAlternative,
    SymbolicValue,
};
use stillwind_source::{CompileError, Spanned};

use crate::state::MacroExtractor;

impl<'a> Visit<'a> for MacroExtractor<'a> {
    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        let element_id = self.next_jsx_element;
        self.next_jsx_element += 1;
        self.jsx_path.push(element_id);
        walk::walk_jsx_element(self, element);
        self.jsx_path.pop();
    }

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
                            "SHJS008",
                            "a module cannot mix framework style macros",
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
            if let Argument::SpreadElement(spread) = argument {
                self.diagnostics.push(CompileError::new(
                    "SHJS002",
                    "style() does not support spread arguments",
                    self.source_span(spread.span),
                ));
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
                self.handle_symbols.insert(symbol_id, definition.clone());
                let scopes = definition.scope.as_ref().map(|scope| {
                    SymbolicValue::exact(self.identity(AssetKind::Scope, &scope.name))
                });
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Refs(vec![RefAlternative {
                        reference: self.identity(AssetKind::Ref, &definition.name),
                        permitted_scopes: scopes,
                    }]),
                );
                self.handle_definitions.push(definition);
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
                        self.identity(AssetKind::Scope, &definition.name),
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
                        self.identity(AssetKind::Animation, &definition.name),
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
                        self.identity(AssetKind::ViewTransition, &definition.name),
                    )),
                );
                self.view_transition_declarations.push(definition);
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}
