//! Declaration-site asset identity.
//!
//! An [`IdentityId`] names one asset by where it was declared, never by where it is
//! used: the kind, the module identity, the declaration span and the local
//! binding name. Importing, re-exporting or aliasing the value therefore keeps the same
//! identity, which is what lets the compiler emit one stable CSS name per declaration.
//!
//! Generated short names are derived from this key later. Keeping the full key here is
//! deliberate: it is what lets a build adapter detect a hash collision instead of
//! silently merging two assets.

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
}

/// Full declaration-site identity. The generated 32-bit name is derived later;
/// retaining this key lets the build adapter detect hash collisions.
///
/// Fields are compared exactly, and the derived order (kind, module identity, span, binding
/// name) is used to sort and deduplicate candidate sets. It is not source order, so
/// consumers must not present it as such.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityId {
    /// Asset kind, which also separates namespaces of derived names.
    kind: AssetKind,
    /// Host module identity of the declaring module.
    module_id: ModuleId,
    /// UTF-8 byte offset where the declaration starts in the original source.
    start: u32,
    /// UTF-8 byte offset where the declaration ends, exclusive.
    end: u32,
    /// Local binding name at the declaration site. Empty for `style()` call sites,
    /// which are identified by their call span instead.
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

    /// Returns the declaration's inclusive UTF-8 byte start.
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Returns the declaration's exclusive UTF-8 byte end.
    pub const fn end(&self) -> u32 {
        self.end
    }

    /// Returns the declaration's local binding name, or an empty name for anonymous calls.
    pub const fn binding_name(&self) -> &LeanStr {
        &self.binding_name
    }

    /// Consumes the identity and returns its declaration coordinates and binding.
    pub fn into_parts(self) -> (AssetKind, ModuleId, u32, u32, LeanStr) {
        (
            self.kind,
            self.module_id,
            self.start,
            self.end,
            self.binding_name,
        )
    }

    /// Builds an identity, preserving the supplied module identity.
    ///
    /// Callers must pass the declaration's own span and binding. Deriving an identity
    /// from a use site would make the asset change meaning as soon as it is imported
    /// under another name.
    pub fn new(
        kind: AssetKind,
        module_id: ModuleId,
        start: u32,
        end: u32,
        binding_name: impl Into<LeanStr>,
    ) -> Self {
        Self {
            kind,
            module_id,
            start,
            end,
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
                4,
                8,
                "menu"
            ),
            IdentityId::new(
                AssetKind::Ref,
                ModuleId::from("src/PopoverMenu.tsx"),
                4,
                8,
                "menu"
            )
        );
    }
}
