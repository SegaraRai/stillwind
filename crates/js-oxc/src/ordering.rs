//! Recognition of imported values that order CSS between modules.
//!
//! A module that uses a value from another module places its CSS after the CSS of the
//! module that declares the value. A caller that passes styles to an imported component,
//! or builds a style on an imported base, then wins over the component or the base when
//! the other cascade criteria are equal. This module records each import binding that
//! the module reads as a value as an [`OrderingUse`]. The host resolves each import to
//! its declaring module after following re-exports.
//!
//! Type-only imports, imports that the module never reads, side-effect imports, and
//! macro imports produce no use.
//!
//! The module also lists the names that a module exports from its own bindings, so the
//! host can follow an `export *` re-export only into the module that exports a name.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{Declaration, ImportDeclarationSpecifier, Program, Statement};
use oxc::semantic::{Reference, Scoping};

use stillwind_ir::OrderingUse;
use stillwind_semantic::ImportBinding;
use stillwind_source::Span;

use crate::state::MacroExtractor;

/// Returns the value imports of `program`, in source order.
///
/// `source` creates the spans of the import specifiers, or returns `None` for a
/// specifier that has no original position, such as an import that component
/// conversion generates. Such a specifier produces no use. A namespace import produces
/// one use whose imported name is `*`.
pub(crate) fn ordering_uses(
    program: &Program<'_>,
    scoping: &Scoping,
    source: impl Fn(oxc::span::Span) -> Option<Span>,
) -> Vec<OrderingUse> {
    let mut uses = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(declaration) = statement else {
            continue;
        };
        if declaration.import_kind.is_type() || MacroExtractor::is_style_import(declaration) {
            continue;
        }
        let Some(module) = declaration.source.value.as_str() else {
            continue;
        };
        for specifier in declaration.specifiers.iter().flatten() {
            let (local, imported) = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    if specifier.import_kind.is_type() {
                        continue;
                    }
                    (
                        &specifier.local,
                        MacroExtractor::module_export_name(&specifier.imported),
                    )
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    (&specifier.local, "default".to_lean_str())
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    (&specifier.local, "*".to_lean_str())
                }
            };
            let read = local.symbol_id.get().is_some_and(|symbol| {
                scoping
                    .get_resolved_references(symbol)
                    .any(Reference::is_value)
            });
            if read && let Some(span) = source(local.span) {
                uses.push(OrderingUse {
                    source: module.to_lean_str(),
                    imported,
                    span,
                });
            }
        }
    }
    uses
}

/// Returns the imports that `uses` read, deduplicated and sorted by source and imported
/// name.
///
/// The local name is the name of the first matching import in `imports`, or empty when
/// no import matches.
pub(crate) fn ordering_imports(
    uses: &[OrderingUse],
    imports: &[ImportBinding],
) -> Vec<ImportBinding> {
    uses.iter()
        .map(|use_| (use_.source.clone(), use_.imported.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|(source, imported)| {
            let local = imports
                .iter()
                .find(|import| import.source == source && import.imported == imported)
                .map_or_default(|import| import.local.clone());
            ImportBinding {
                local,
                source,
                imported,
            }
        })
        .collect()
}

/// Returns the names that `program` exports from its own bindings, sorted and
/// deduplicated.
///
/// The names come from `export` declarations, `export { local as name }` lists, and
/// `export default`, which adds `default`. Type-only exports are left out. Re-exports
/// with a `from` clause are left out, because the summary records them as re-exports.
pub(crate) fn local_exports(program: &Program<'_>) -> Vec<LeanStr> {
    let mut names = BTreeSet::new();
    for statement in &program.body {
        match statement {
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::VariableDeclaration(variables) => {
                    for declarator in &variables.declarations {
                        names.extend(
                            declarator
                                .id
                                .get_binding_identifiers()
                                .into_iter()
                                .map(|identifier| identifier.name.to_lean_str()),
                        );
                    }
                }
                Declaration::FunctionDeclaration(function) => {
                    names.extend(function.id.as_ref().map(|id| id.name.to_lean_str()));
                }
                Declaration::ClassDeclaration(class) => {
                    names.extend(class.id.as_ref().map(|id| id.name.to_lean_str()));
                }
                Declaration::TSEnumDeclaration(declaration) => {
                    names.insert(declaration.id.name.to_lean_str());
                }
                Declaration::TSNamespaceDeclaration(declaration) => {
                    names.insert(declaration.id.name.to_lean_str());
                }
                _ => {}
            },
            Statement::ExportNamedDeclaration(export) if !export.export_kind.is_type() => {
                names.extend(
                    export
                        .specifiers
                        .iter()
                        .filter(|specifier| !specifier.export_kind.is_type())
                        .map(|specifier| MacroExtractor::module_export_name(&specifier.exported)),
                );
            }
            Statement::ExportDefaultDeclaration(_) => {
                names.insert("default".to_lean_str());
            }
            _ => {}
        }
    }
    names.into_iter().collect()
}
