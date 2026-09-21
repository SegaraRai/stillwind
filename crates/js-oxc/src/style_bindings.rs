//! Collect direct producer bindings before extracting their dependencies.
//!
//! Seeding runs before the main visit so that `const props = style("…")` is already a
//! known style value when the visit reaches `style(props)`. Only direct bindings of a
//! macro call become values here; a binding that is ever assigned to is dropped,
//! because its value at a use site is no longer known.
//!
//! Names are compared through symbols, so a shadowing parameter cannot be confused with
//! the macro import.

use std::collections::{HashMap, HashSet};

use oxc::ast::ast::{
    BindingPattern, Expression, IdentifierReference, ImportDeclarationSpecifier, ModuleExportName,
    Program, Statement, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};

use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue};
use stillwind_source::{SourceFile, Span};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Collects the symbols bound to the `style` macro and the direct style bindings
    /// they create.
    ///
    /// Type-only imports are skipped, so `import type { style }` never makes a value
    /// call look like the macro.
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
            source: self.source,
            coordinates: self.coordinates,
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

/// Visitor state for one seeding pass.
struct Bindings<'s> {
    /// Symbols bound to the imported `style` macro.
    macros: &'s HashSet<SymbolId>,
    /// Scoping table used to resolve callee references.
    scoping: &'s Scoping,
    /// Analysis source that owns Oxc byte spans.
    source: &'s SourceFile,
    /// Maps analysis UTF-8 spans to original UTF-8 spans before creating identities.
    coordinates: &'s dyn Fn(Span) -> Option<Span>,
    /// Resolved value for each direct style binding.
    values: HashMap<SymbolId, ExportedValue>,
    /// Symbols that are ever written to, and therefore not constants.
    writes: HashSet<SymbolId>,
}

impl<'a> Visit<'a> for Bindings<'_> {
    /// Records `const x = style(...)` as a direct style value.
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
            && let Some(span) =
                (self.coordinates)(Span::new(self.source.id(), call.span.start, call.span.end))
        {
            self.values.insert(
                binding,
                ExportedValue::Styles(SymbolicValue::exact(IdentityId::new(
                    AssetKind::Style,
                    self.source.path(),
                    span.start.0,
                    span.end.0,
                    "",
                ))),
            );
        }
        walk::walk_variable_declarator(self, declaration);
    }

    /// Records written symbols so reassigned bindings are removed afterwards.
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
