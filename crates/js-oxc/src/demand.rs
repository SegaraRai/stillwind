//! Conservative declaration-level slicing for host semantic reads.

use std::collections::{HashMap, HashSet};

use oxc::ast::ast::{BindingIdentifier, IdentifierReference, Program};
use oxc::ast_visit::Visit;
use oxc::semantic::{Scoping, SymbolId};
use stillwind_semantic::{ImportBinding, ModuleSummary};

#[derive(Default)]
struct References {
    bindings: Vec<(String, SymbolId)>,
    references: Vec<(u32, u32, oxc::semantic::ReferenceId)>,
}

impl<'a> Visit<'a> for References {
    fn visit_binding_identifier(&mut self, node: &BindingIdentifier<'a>) {
        if let Some(symbol) = node.symbol_id.get() {
            self.bindings.push((node.name.to_string(), symbol));
        }
    }

    fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
        if let Some(reference) = node.reference_id.get() {
            self.references
                .push((node.span.start, node.span.end, reference));
        }
    }
}

pub(super) fn collect(
    program: &Program<'_>,
    scoping: &Scoping,
    probes: &std::collections::BTreeSet<(u32, u32)>,
    exports: &[(SymbolId, String)],
    summary: &mut ModuleSummary,
) {
    let mut dependencies = HashMap::<SymbolId, Vec<SymbolId>>::new();
    let mut imports = HashMap::<SymbolId, ImportBinding>::new();
    let mut roots = Vec::new();
    for statement in &program.body {
        let mut references = References::default();
        references.visit_statement(statement);
        let symbols: Vec<_> = references
            .references
            .iter()
            .filter_map(|(_, _, reference)| scoping.get_reference(*reference).symbol_id())
            .collect();
        for (start, end, reference) in &references.references {
            if probes
                .iter()
                .any(|(left, right)| left <= start && end <= right)
                && let Some(symbol) = scoping.get_reference(*reference).symbol_id()
            {
                roots.push(symbol);
            }
        }
        for (_, _, reference) in &references.references {
            let reference = scoping.get_reference(*reference);
            if (reference.is_write() || references.bindings.is_empty())
                && let Some(symbol) = reference.symbol_id()
            {
                dependencies.entry(symbol).or_default().extend(&symbols);
            }
        }
        for (name, symbol) in references.bindings {
            // Only an import statement may associate a binding by spelling.
            // Other bindings (including shadowed parameters) retain their symbol IDs.
            if matches!(statement, oxc::ast::ast::Statement::ImportDeclaration(_))
                && let Some(import) = summary.imports.iter().find(|import| import.local == name)
            {
                imports.insert(symbol, import.clone());
            }
            dependencies.entry(symbol).or_default().extend(&symbols);
        }
    }
    let slice = |roots: Vec<SymbolId>| {
        let mut pending = roots;
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        while let Some(symbol) = pending.pop() {
            if !visited.insert(symbol) {
                continue;
            }
            if let Some(import) = imports.get(&symbol) {
                result.push(import.clone());
            }
            if let Some(references) = dependencies.get(&symbol) {
                pending.extend(references);
            }
        }
        result.sort_by(|a, b| {
            (&a.source, &a.imported, &a.local).cmp(&(&b.source, &b.imported, &b.local))
        });
        result
    };
    summary.probe_imports = slice(roots);
    for (symbol, name) in exports {
        summary
            .export_imports
            .insert(name.clone(), slice(vec![*symbol]));
    }
}
