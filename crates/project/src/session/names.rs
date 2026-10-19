//! Checks generated names against published symbols and authored CSS namespaces.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;

use stillwind_compiler::{GeneratedSymbol, IdentifierPool};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::ModuleId;

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

/// Owner identity recorded for names that authored CSS uses. No generated symbol has
/// this identity, so every generated name equal to an authored name collides.
const AUTHORED_IDENTITY: &str = "<authored CSS>";

/// Published symbol owners by pool, name, and module.
///
/// For each module, the index keeps the identity of the module's last symbol with the
/// name. The update methods must receive the symbols of the artifact that the session
/// publishes or removes, so the index matches the published artifacts.
#[derive(Clone, Debug, Default)]
pub(super) struct SymbolIndex {
    owners: BTreeMap<IdentifierPool, BTreeMap<LeanStr, BTreeMap<ModuleId, LeanStr>>>,
}

impl SymbolIndex {
    /// Records `symbols` of the artifact published for `module`. The caller must first
    /// remove the symbols of the artifact that the new artifact replaces.
    pub(super) fn insert(&mut self, module: &ModuleId, symbols: &[GeneratedSymbol]) {
        for symbol in symbols {
            self.owners
                .entry(symbol.pool)
                .or_default()
                .entry(symbol.name.clone())
                .or_default()
                .insert(module.clone(), symbol.identity.clone());
        }
    }

    /// Removes the entries of `module` for `symbols` of its removed artifact.
    pub(super) fn remove(&mut self, module: &ModuleId, symbols: &[GeneratedSymbol]) {
        for symbol in symbols {
            let Some(names) = self.owners.get_mut(&symbol.pool) else {
                continue;
            };
            if let Some(modules) = names.get_mut(symbol.name.as_str()) {
                modules.remove(module);
                if modules.is_empty() {
                    names.remove(symbol.name.as_str());
                }
            }
        }
    }

    /// Returns the identity that owns `name` in `pool` outside `except`.
    ///
    /// When several modules own the name, the module that sorts last wins. This keeps
    /// the precedence of an index that inserts all published symbols in module order.
    fn owner(&self, pool: IdentifierPool, name: &str, except: &ModuleId) -> Option<&str> {
        self.owners
            .get(&pool)?
            .get(name)?
            .iter()
            .rev()
            .find(|(module, _)| *module != except)
            .map(|(_, identity)| identity.as_str())
    }
}

/// Checks the symbols of one module against the rest of the document.
///
/// The module's earlier symbols take precedence over authored names, and authored names
/// take precedence over symbols of other published modules. The check looks up only the
/// inserted names and does not read the symbols of other modules.
pub(super) struct ModuleNames<'a> {
    module: &'a ModuleId,
    published: &'a SymbolIndex,
    reserved: Vec<&'a ReservedNames>,
    own: BTreeMap<(IdentifierPool, &'a str), &'a str>,
}

impl<'a> ModuleNames<'a> {
    /// Creates a check for `module`. `published` must index every published artifact,
    /// and `reserved` must supply the authored names of the whole document.
    pub(super) fn new(
        module: &'a ModuleId,
        published: &'a SymbolIndex,
        reserved: impl Iterator<Item = &'a ReservedNames>,
    ) -> Self {
        Self {
            module,
            published,
            reserved: reserved.collect(),
            own: BTreeMap::new(),
        }
    }

    /// Records a symbol and returns a collision when its name belonged to another
    /// identity.
    pub(super) fn insert(&mut self, symbol: &'a GeneratedSymbol) -> Option<CompileDiagnostic> {
        let name = symbol.name.as_str();
        let previous = self
            .own
            .insert((symbol.pool, name), symbol.identity.as_str())
            .or_else(|| {
                self.reserved
                    .iter()
                    .filter_map(|reserved| reserved_pool(reserved, symbol.pool))
                    .any(|names| names.contains(name))
                    .then_some(AUTHORED_IDENTITY)
            })
            .or_else(|| self.published.owner(symbol.pool, name, self.module));
        previous
            .filter(|identity| *identity != symbol.identity)
            .map(|_| CompileDiagnostic::GeneratedIdentifierCollision { name: name.into() })
    }
}

/// Returns the names that `reserved` records for `pool`, or `None` for a pool that
/// authored CSS cannot use.
fn reserved_pool(reserved: &ReservedNames, pool: IdentifierPool) -> Option<&BTreeSet<LeanStr>> {
    match pool {
        IdentifierPool::CustomProperty => Some(&reserved.custom_properties),
        IdentifierPool::LayerName => Some(&reserved.layers),
        IdentifierPool::KeyframesName => Some(&reserved.keyframes),
        IdentifierPool::ClassToken => Some(&reserved.classes),
        IdentifierPool::AnchorName => Some(&reserved.anchor_names),
        IdentifierPool::ContainerName => Some(&reserved.container_names),
        IdentifierPool::ViewTransitionClass => Some(&reserved.view_transition_classes),
        IdentifierPool::RuleBoundary => None,
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
    for pool in [
        IdentifierPool::CustomProperty,
        IdentifierPool::LayerName,
        IdentifierPool::KeyframesName,
        IdentifierPool::ClassToken,
        IdentifierPool::AnchorName,
        IdentifierPool::ContainerName,
        IdentifierPool::ViewTransitionClass,
    ] {
        for name in reserved_pool(reserved, pool).into_iter().flatten() {
            names.insert((pool, name.as_str()), AUTHORED_IDENTITY);
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
