//! Node asset kinds and declaration identities, converted without host I/O.

use napi_derive::napi;
use serde::Serialize;

use stillwind_semantic::{AssetKind, IdentityId};

/// Asset kinds as the host names them.
#[napi(string_enum = "kebab-case")]
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAssetKind {
    /// A `style(...)` call's payload.
    Style,
    /// A `style.subtree()` declaration; the host sees the kind `scope`.
    Scope,
    /// A `style.ref()` declaration.
    Ref,
    /// A `style.animation()` declaration.
    Animation,
    /// A `style.viewTransition()` declaration.
    ViewTransition,
}

impl From<NativeAssetKind> for AssetKind {
    fn from(value: NativeAssetKind) -> Self {
        match value {
            NativeAssetKind::Style => Self::Style,
            NativeAssetKind::Scope => Self::Subtree,
            NativeAssetKind::Ref => Self::Ref,
            NativeAssetKind::Animation => Self::Animation,
            NativeAssetKind::ViewTransition => Self::ViewTransition,
        }
    }
}

impl From<AssetKind> for NativeAssetKind {
    fn from(value: AssetKind) -> Self {
        match value {
            AssetKind::Style => Self::Style,
            AssetKind::Subtree => Self::Scope,
            AssetKind::Ref => Self::Ref,
            AssetKind::Animation => Self::Animation,
            AssetKind::ViewTransition => Self::ViewTransition,
        }
    }
}

/// Identity of a declaration site.
#[napi(object)]
#[derive(Serialize)]
pub struct NativeIdentity {
    /// Kind of declaration this identity names.
    pub kind: NativeAssetKind,
    /// Normalized module path of the declaring module.
    #[napi(js_name = "module_path")]
    pub module_path: String,
    /// Byte start of the declaration identity: binding-name span for named assets,
    /// call span for styles.
    pub start: u32,
    /// Exclusive byte end of the declaration identity span.
    pub end: u32,
    /// Lexical binding name, or the empty string for anonymous calls.
    #[napi(js_name = "binding_name")]
    pub binding_name: String,
}

impl From<NativeIdentity> for IdentityId {
    fn from(value: NativeIdentity) -> Self {
        Self::new(
            value.kind.into(),
            &value.module_path,
            value.start,
            value.end,
            value.binding_name,
        )
    }
}

impl From<IdentityId> for NativeIdentity {
    fn from(value: IdentityId) -> Self {
        Self {
            kind: value.kind.into(),
            module_path: value.module_path,
            start: value.start,
            end: value.end,
            binding_name: value.binding_name,
        }
    }
}
