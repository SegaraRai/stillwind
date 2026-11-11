//! Collects direct producer bindings before extracting their dependencies.
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
    BindingPattern, Expression, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
    ModuleExportName, Program, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};

use stillwind_ir::{OutputFormat, from_macro_import};
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
        let mut collector = MacroBindings {
            outputs: HashMap::new(),
        };
        collector.visit_program(program);
        let macros = collector.outputs;
        let mut collector = Bindings {
            macros: &macros,
            scoping: self.scoping,
            source: self.source,
            coordinates: self.coordinates,
            calls: Vec::new(),
            writes: HashSet::new(),
        };
        collector.visit_program(program);
        for (symbol, call) in collector.calls {
            if collector.writes.contains(&symbol) {
                continue;
            }
            let identity = IdentityId::new(
                AssetKind::Style,
                self.module_id.clone(),
                self.style_key(call),
                "",
            );
            self.local_values.insert(
                symbol,
                ExportedValue::Styles(SymbolicValue::exact(identity)),
            );
        }
        self.style_symbols.extend(macros);
    }
}

/// Macro bindings from all scopes, including projected client scripts.
struct MacroBindings {
    outputs: HashMap<SymbolId, OutputFormat>,
}

impl<'a> Visit<'a> for MacroBindings {
    fn visit_import_declaration(&mut self, import: &ImportDeclaration<'a>) {
        if import.import_kind.is_type() {
            return;
        }
        let Some(spec) = import.source.value.as_str().and_then(from_macro_import) else {
            return;
        };
        for specifier in import.specifiers.iter().flatten() {
            if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier
                && !specifier.import_kind.is_type()
                && let ModuleExportName::IdentifierName(name) = &specifier.imported
                && name.name == "style"
                && let Some(symbol) = specifier.local.symbol_id.get()
            {
                self.outputs.insert(symbol, spec.output);
            }
        }
    }
}

/// Visitor state for one seeding pass.
struct Bindings<'s> {
    /// Symbols bound to the imported `style` macro.
    macros: &'s HashMap<SymbolId, OutputFormat>,
    /// Scoping table used to resolve callee references.
    scoping: &'s Scoping,
    /// Analysis source that owns Oxc byte spans.
    source: &'s SourceFile,
    /// Maps analysis UTF-8 spans to original UTF-8 spans. A call without a mapping is
    /// not recorded.
    coordinates: &'s dyn Fn(Span) -> Option<Span>,
    /// Direct style bindings and their calls in the analysis buffer, in source order.
    calls: Vec<(SymbolId, Span)>,
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
            && self.macros.contains_key(&symbol)
            && let Some(binding) = identifier.symbol_id.get()
        {
            let span = Span::new(self.source.id(), call.span.start, call.span.end);
            if (self.coordinates)(span).is_some() {
                self.calls.push((binding, span));
            }
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
