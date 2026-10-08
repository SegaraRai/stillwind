//! Checks generated names against published symbols and authored CSS namespaces.

use std::collections::BTreeMap;

use stillwind_compiler::{GeneratedSymbol, IdentifierPool};
use stillwind_diagnostics::CompileDiagnostic;

use crate::ReservedNames;

/// Names already owned by generated declarations or authored CSS in one document.
pub(super) struct DocumentNames<'a> {
    names: BTreeMap<(IdentifierPool, &'a str), &'a str>,
}

impl<'a> DocumentNames<'a> {
    /// Builds the name index from symbols and reserves every authored namespace.
    pub(super) fn new(
        symbols: impl Iterator<Item = &'a GeneratedSymbol>,
        reserved: impl Iterator<Item = &'a ReservedNames>,
    ) -> Self {
        let mut names = symbols
            .map(|symbol| {
                (
                    (symbol.pool, symbol.name.as_str()),
                    symbol.identity.as_str(),
                )
            })
            .collect();
        for authored in reserved {
            insert_reserved(&mut names, authored);
        }
        Self { names }
    }

    /// Records a symbol and returns a collision when its name belonged to another identity.
    pub(super) fn insert(&mut self, symbol: &'a GeneratedSymbol) -> Option<CompileDiagnostic> {
        insert_document_name(&mut self.names, symbol)
    }
}

/// Records every name one authored stylesheet already uses.
///
/// An authored name owns its namespace before anything is generated, so it is recorded
/// with an identity no generated name can have.
fn insert_reserved<'a>(
    names: &mut BTreeMap<(IdentifierPool, &'a str), &'a str>,
    reserved: &'a ReservedNames,
) {
    for (pool, names_in_pool) in [
        (IdentifierPool::CustomProperty, &reserved.custom_properties),
        (IdentifierPool::LayerName, &reserved.layers),
        (IdentifierPool::KeyframesName, &reserved.keyframes),
        (IdentifierPool::ClassToken, &reserved.classes),
        (IdentifierPool::AnchorName, &reserved.anchor_names),
        (IdentifierPool::ContainerName, &reserved.container_names),
        (
            IdentifierPool::ViewTransitionClass,
            &reserved.view_transition_classes,
        ),
    ] {
        for name in names_in_pool {
            names.insert((pool, name.as_str()), "<authored CSS>");
        }
    }
}

/// Records one generated name, reporting it when the namespace already holds another
/// identity.
fn insert_document_name<'a>(
    names: &mut BTreeMap<(IdentifierPool, &'a str), &'a str>,
    symbol: &'a GeneratedSymbol,
) -> Option<CompileDiagnostic> {
    names
        .insert(
            (symbol.pool, symbol.name.as_str()),
            symbol.identity.as_str(),
        )
        .filter(|identity| *identity != symbol.identity)
        .map(|_| CompileDiagnostic::GeneratedIdentifierCollision {
            name: symbol.name.as_str().into(),
        })
}
