//! Node asset kinds and declaration identities, converted without host I/O.

use napi_derive::napi;
use serde::Serialize;

use stillwind_semantic::{AssetKind, DeclarationKey, IdentityId};
use stillwind_source::ModuleId;

/// Asset kinds as the host names them.
#[napi(string_enum = "camelCase")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeAssetKind {
    /// A `style(...)` call's payload.
    Style,
    /// A `style.ref()` declaration, which may also bound other refs.
    Ref,
    /// A `style.keyframes()` declaration.
    Keyframes,
    /// A `style.viewTransition()` declaration.
    ViewTransition,
    /// A `style.when()` declaration.
    StoredVariant,
}

impl From<NativeAssetKind> for AssetKind {
    fn from(value: NativeAssetKind) -> Self {
        match value {
            NativeAssetKind::Style => Self::Style,
            NativeAssetKind::Ref => Self::Ref,
            NativeAssetKind::Keyframes => Self::Keyframes,
            NativeAssetKind::ViewTransition => Self::ViewTransition,
            NativeAssetKind::StoredVariant => Self::StoredVariant,
        }
    }
}

impl From<AssetKind> for NativeAssetKind {
    fn from(value: AssetKind) -> Self {
        match value {
            AssetKind::Style => Self::Style,
            AssetKind::Ref => Self::Ref,
            AssetKind::Keyframes => Self::Keyframes,
            AssetKind::ViewTransition => Self::ViewTransition,
            AssetKind::StoredVariant => Self::StoredVariant,
        }
    }
}

/// Identity of a declaration site.
#[napi(object)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeIdentity {
    /// Kind of declaration this identity names.
    pub kind: NativeAssetKind,
    /// Module identity of the declaring module.
    pub module_id: String,
    /// Names of the enclosing declarations from the outermost to the innermost.
    pub path: Vec<String>,
    /// Zero-based position among earlier declarations of the module with the same
    /// kind, path, and binding name.
    pub ordinal: u32,
    /// Lexical binding name, or the empty string for anonymous calls.
    pub binding_name: String,
}

impl From<NativeIdentity> for IdentityId {
    fn from(value: NativeIdentity) -> Self {
        Self::new(
            value.kind.into(),
            ModuleId::from(value.module_id),
            DeclarationKey::new(
                value.path.into_iter().map(Into::into).collect(),
                value.ordinal,
            ),
            value.binding_name,
        )
    }
}

impl From<IdentityId> for NativeIdentity {
    fn from(value: IdentityId) -> Self {
        let (kind, module_id, key, binding_name) = value.into_parts();
        Self {
            kind: kind.into(),
            module_id: (module_id.into_string()).into(),
            path: key.path.iter().map(ToString::to_string).collect(),
            ordinal: key.ordinal,
            binding_name: binding_name.to_string(),
        }
    }
}
