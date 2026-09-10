use serde::{Deserialize, Serialize};

use stillwind_source::normalized_module_path;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    Scope,
    Ref,
    Animation,
    ViewTransition,
}

/// Full declaration-site identity. The generated 32-bit name is derived later;
/// retaining this key lets the build adapter detect hash collisions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct IdentityId {
    pub kind: AssetKind,
    pub module_path: String,
    pub start: u32,
    pub end: u32,
    pub binding_name: String,
}

impl IdentityId {
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
