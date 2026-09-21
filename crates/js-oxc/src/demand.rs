//! Conservative declaration-level slicing for host semantic reads.
//!
//! A host type checker should not be asked about a whole module when one expression
//! needs its type. This module walks each top-level statement once, records which
//! symbols that statement reads and which bindings it declares, and then answers two
//! questions: which imports the module's type probes can reach, and which imports each
//! exported binding needs.
//!
//! The slice is deliberately conservative. It follows symbol references, not control
//! flow, so a statement that merely mentions a symbol contributes it, and it keeps
//! every import reachable from a probe rather than guessing which branch matters. The
//! result can be larger than necessary, but never smaller.

use std::collections::{BTreeSet, HashMap, HashSet};

use oxc::ast::ast::{
    BindingIdentifier, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier, Program,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{ReferenceId, Scoping, SymbolId};

use stillwind_ir::from_macro_import;
use stillwind_semantic::{ImportBinding, ModuleSummary};

/// The bindings and references of one top-level statement.
#[derive(Default)]
struct StatementReads {
    /// Bound names paired with their symbols, in source order.
    bindings: Vec<(String, SymbolId)>,
    /// Identifier reads as span plus reference id, in source order.
    references: Vec<(u32, u32, ReferenceId)>,
    /// Non-macro import bindings paired with their local symbols, including imports
    /// nested in converter-generated functions. Symbol identity distinguishes equal
    /// local names in separate scopes when resolving semantic dependencies.
    imports: Vec<(SymbolId, ImportBinding)>,
}

impl<'a> Visit<'a> for StatementReads {
    /// Records non-macro specifiers with resolved local symbols as dependency targets,
    /// then walks the declaration to collect its bindings. Default and namespace
    /// imports use `default` and `*` as their imported names.
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if from_macro_import(declaration.source.value.as_str()).is_none() {
            for specifier in declaration.specifiers.iter().flatten() {
                let (local, imported) = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        (&specifier.local, specifier.imported.name().to_string())
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        (&specifier.local, "default".into())
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (&specifier.local, "*".into())
                    }
                };
                if let Some(symbol) = local.symbol_id.get() {
                    self.imports.push((
                        symbol,
                        ImportBinding {
                            source: declaration.source.value.to_string(),
                            imported,
                            local: local.name.to_string(),
                        },
                    ));
                }
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    /// Records one declared binding.
    fn visit_binding_identifier(&mut self, node: &BindingIdentifier<'a>) {
        if let Some(symbol) = node.symbol_id.get() {
            self.bindings.push((node.name.to_string(), symbol));
        }
    }

    /// Records one identifier read with its span.
    fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
        if let Some(reference) = node.reference_id.get() {
            self.references
                .push((node.span.start, node.span.end, reference));
        }
    }
}

/// Fills [`ModuleSummary::probe_imports`] and [`ModuleSummary::export_imports`] on
/// `summary`.
///
/// `probes` are the spans the project asked about and `exports` the
/// `(symbol, exported name)` pairs to slice for. Both results are sorted by
/// `(source, imported, local)`, so callers must not read them as source order.
pub(crate) fn collect(
    program: &Program<'_>,
    scoping: &Scoping,
    probes: &BTreeSet<(u32, u32)>,
    exports: &[(SymbolId, String)],
    summary: &mut ModuleSummary,
) {
    let mut dependencies = HashMap::<SymbolId, Vec<SymbolId>>::new();
    let mut imports = HashMap::<SymbolId, ImportBinding>::new();
    let mut roots = Vec::new();
    for statement in &program.body {
        let mut references = StatementReads::default();
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
        imports.extend(references.imports);
        for (_, symbol) in references.bindings {
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
