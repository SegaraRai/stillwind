//! Declaration-site asset records published with a module summary.
//!
//! An [`AssetRecord`] is the serializable form of one asset: the [`IdentityId`] created
//! at its declaration site, the author-visible metadata, and the byte span of the
//! producing call in the module's original source. The host stores records next to
//! imports so a value that crossed a module boundary can still be resolved back to the
//! declaration that owns it.
//!
//! Records are plain values: nothing here registers, deduplicates or validates them,
//! and a record must not be used to identify an asset without its
//! [`AssetRecord::identity`] (labels are not unique).

use serde::{Deserialize, Serialize};

use super::identity::IdentityId;

/// Author-visible metadata for one asset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Optional literal name the author passed to the producer, such as
    /// `style.ref("trigger")`. Display and marker-naming input only: identity
    /// never depends on it, so equal labels stay distinct assets.
    pub label: Option<String>,
}

/// One declared asset, keyed by its declaration-site identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    /// Stable key of the asset; the only field that identifies it.
    pub identity: IdentityId,
    /// Optional author-provided label.
    pub metadata: AssetMetadata,
    /// Byte offset where the producing call starts in the module's original source.
    pub call_start: u32,
    /// Byte offset where the producing call ends, exclusive.
    pub call_end: u32,
}
