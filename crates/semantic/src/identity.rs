//! Declaration-site asset identity.
//!
//! An [`IdentityId`] names one asset by where it was declared, never by where it is
//! used: the kind, the module identity, the [`DeclarationKey`] and the local
//! binding name. Importing, re-exporting or aliasing the value therefore keeps the same
//! identity, which is what lets the compiler emit one stable CSS name per declaration.
//!
//! Generated short names are derived from this key later. The full key stays here so
//! that a build adapter can detect a hash collision instead of silently merging two
//! assets.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::ModuleId;

/// Which kind of Stillwind asset an identity denotes.
///
/// The kind is part of the identity, so two assets declared at the same span stay
/// distinct. Serialized in camelCase; that spelling is part of the host protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    /// A `style()` call site; its span is the call itself.
    Style,
    /// A `style.ref()` declaration: a DOM identity that may also bound other refs.
    Ref,
    /// A `style.keyframes()` asset.
    Keyframes,
    /// A `style.viewTransition()` named view-transition asset.
    ViewTransition,
    /// A `style.when()` declaration: a stored variant sequence that `style()` keys use.
    StoredVariant,
}

/// Position of a declaration among the declarations of its module, independent of byte
/// offsets.
///
/// The path names the declarations that enclose this one: named functions, classes,
/// methods, variable bindings, and object keys, from the outermost to the innermost.
/// Anonymous functions add no segment. The ordinal counts earlier declarations in
/// source order that have the same kind, path, and binding name. An edit outside the
/// enclosing declarations therefore keeps the key, and an edit inside them changes only
/// the keys of later declarations with the same path.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarationKey {
    /// Names of the enclosing declarations from the outermost to the innermost.
    pub path: Vec<LeanStr>,
    /// Zero-based position among earlier declarations with the same kind, path, and
    /// binding name.
    pub ordinal: u32,
}

impl DeclarationKey {
    /// Creates a key from enclosing declaration names and an ordinal.
    pub fn new(path: Vec<LeanStr>, ordinal: u32) -> Self {
        Self { path, ordinal }
    }

    /// Creates a key for a declaration with no enclosing named declaration.
    pub const fn top(ordinal: u32) -> Self {
        Self {
            path: Vec::new(),
            ordinal,
        }
    }
}

/// Full declaration-site identity. The generated 32-bit name is derived later;
/// retaining this key lets the build adapter detect hash collisions.
///
/// The identity contains no byte offsets, so an edit elsewhere in the declaring module
/// keeps it. Fields are compared exactly, and the derived order (kind, module identity,
/// key, binding name) is used to sort and deduplicate candidate sets. It is not source
/// order, so consumers must not present it as such.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityId {
    /// Asset kind, which also separates namespaces of derived names.
    kind: AssetKind,
    /// Host module identity of the declaring module.
    module_id: ModuleId,
    /// Enclosing declarations and ordinal, serialized as `path` and `ordinal` fields.
    #[serde(flatten)]
    key: DeclarationKey,
    /// Local binding name at the declaration site. Empty for `style()` call sites,
    /// which are identified by their key alone.
    binding_name: LeanStr,
}

impl IdentityId {
    /// Returns the declaration's asset kind.
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Returns the module identity of the declaration.
    pub const fn module_id(&self) -> &ModuleId {
        &self.module_id
    }

    /// Returns the declaration's enclosing path and ordinal.
    pub const fn key(&self) -> &DeclarationKey {
        &self.key
    }

    /// Returns the declaration's local binding name, or an empty name for anonymous calls.
    pub const fn binding_name(&self) -> &LeanStr {
        &self.binding_name
    }

    /// Consumes the identity and returns its kind, module, key, and binding.
    pub fn into_parts(self) -> (AssetKind, ModuleId, DeclarationKey, LeanStr) {
        (self.kind, self.module_id, self.key, self.binding_name)
    }

    /// Builds an identity, preserving the supplied module identity.
    ///
    /// Callers must pass the declaration's own key and binding. Deriving an identity
    /// from a use site would make the asset change meaning as soon as it is imported
    /// under another name.
    pub fn new(
        kind: AssetKind,
        module_id: ModuleId,
        key: DeclarationKey,
        binding_name: impl Into<LeanStr>,
    ) -> Self {
        Self {
            kind,
            module_id,
            key,
            binding_name: binding_name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_preserve_host_module_ids() {
        assert_ne!(
            IdentityId::new(
                AssetKind::Ref,
                ModuleId::from(r"src\PopoverMenu.tsx"),
                DeclarationKey::top(0),
                "menu"
            ),
            IdentityId::new(
                AssetKind::Ref,
                ModuleId::from("src/PopoverMenu.tsx"),
                DeclarationKey::top(0),
                "menu"
            )
        );
    }
}
