//! Declaration-site asset identity.
//!
//! An [`IdentityId`] names one asset by where it was declared, never by where it is
//! used: the kind, the normalized module path, the declaration span and the local
//! binding name. Importing, re-exporting or aliasing the value therefore keeps the same
//! identity, which is what lets the compiler emit one stable CSS name per declaration.
//!
//! Generated short names are derived from this key later. Keeping the full key here is
//! deliberate: it is what lets a build adapter detect a hash collision instead of
//! silently merging two assets.

use serde::{Deserialize, Serialize};

use stillwind_source::normalized_module_path;

/// Which kind of Stillwind asset an identity denotes.
///
/// The kind is part of the identity, so a ref and a scope declared at the same span
/// stay distinct. Serialized in kebab-case; that spelling is part of the host protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    /// A `style()` call site; its span is the call itself.
    Style,
    /// A `style.scope()` boundary.
    Scope,
    /// A `style.ref()` declaration, optionally bound to a scope.
    Ref,
    /// A `style.animation()` keyframe asset.
    Animation,
    /// A `style.viewTransition()` named view-transition asset.
    ViewTransition,
}

/// Full declaration-site identity. The generated 32-bit name is derived later;
/// retaining this key lets the build adapter detect hash collisions.
///
/// Fields are compared exactly, and the derived order (kind, module path, span, binding
/// name) is used to sort and deduplicate candidate sets. It is not source order, so
/// consumers must not present it as such.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct IdentityId {
    /// Asset kind, which also separates namespaces of derived names.
    pub kind: AssetKind,
    /// Normalized project-relative path of the declaring module.
    pub module_path: String,
    /// Byte offset where the declaration starts in the original source.
    pub start: u32,
    /// Byte offset where the declaration ends, exclusive.
    pub end: u32,
    /// Local binding name at the declaration site. Empty for `style()` call sites,
    /// which are identified by their call span instead.
    pub binding_name: String,
}

impl IdentityId {
    /// Builds an identity, normalizing `module_path` with [`normalized_module_path`].
    ///
    /// Callers must pass the declaration's own span and binding. Deriving an identity
    /// from a use site would make the asset change meaning as soon as it is imported
    /// under another name.
    pub fn new(
        kind: AssetKind,
        module_path: &str,
        start: u32,
        end: u32,
        binding_name: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            module_path: normalized_module_path(module_path),
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
    fn identities_are_path_stable_across_operating_systems() {
        assert_eq!(
            IdentityId::new(AssetKind::Ref, r"src\Menu.tsx", 4, 8, "menu"),
            IdentityId::new(AssetKind::Ref, "src/Menu.tsx", 4, 8, "menu")
        );
    }
}
