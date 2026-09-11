//! Collect direct producer bindings before extracting their dependencies.

use std::collections::{HashMap, HashSet};

use oxc::ast::ast::{
    BindingPattern, Expression, IdentifierReference, ImportDeclarationSpecifier, ModuleExportName,
    Program, Statement, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};

use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn seed_style_bindings(&mut self, program: &Program<'a>) {
        let mut macros = HashSet::new();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind.is_type() || !Self::is_style_import(import) {
                continue;
            }
            for specifier in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier
                    && !specifier.import_kind.is_type()
                    && let ModuleExportName::IdentifierName(name) = &specifier.imported
                    && name.name == "style"
                    && let Some(symbol) = specifier.local.symbol_id.get()
                {
                    macros.insert(symbol);
                }
            }
        }
        let mut collector = Bindings {
            macros: &macros,
            scoping: self.scoping,
            module: self.source.path(),
            values: HashMap::new(),
            writes: HashSet::new(),
        };
        collector.visit_program(program);
        collector
            .values
            .retain(|symbol, _| !collector.writes.contains(symbol));
        self.local_values.extend(collector.values);
        self.style_symbols.extend(macros);
    }
}

struct Bindings<'s> {
    macros: &'s HashSet<SymbolId>,
    scoping: &'s Scoping,
    module: &'s str,
    values: HashMap<SymbolId, ExportedValue>,
    writes: HashSet<SymbolId>,
}

impl<'a> Visit<'a> for Bindings<'_> {
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declaration.id, &declaration.init)
            && let Expression::Identifier(callee) = &call.callee
            && let Some(reference) = callee.reference_id.get()
            && let Some(symbol) = self.scoping.get_reference(reference).symbol_id()
            && self.macros.contains(&symbol)
            && let Some(binding) = identifier.symbol_id.get()
        {
            self.values.insert(
                binding,
                ExportedValue::Styles(SymbolicValue::exact(IdentityId::new(
                    AssetKind::Style,
                    self.module,
                    call.span.start,
                    call.span.end,
                    "",
                ))),
            );
        }
        walk::walk_variable_declarator(self, declaration);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(reference) = identifier.reference_id.get() {
            let reference = self.scoping.get_reference(reference);
            if reference.is_write()
                && let Some(symbol) = reference.symbol_id()
            {
                self.writes.insert(symbol);
            }
        }
    }
}
