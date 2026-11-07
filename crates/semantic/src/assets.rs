//! Declaration-site asset records published with a module summary.
//!
//! An [`AssetRecord`] is the serializable form of one asset: the [`IdentityId`] created
//! at its declaration site, the author-visible metadata, and the byte span of the
//! producing call in the module's original source. The host stores records next to
//! imports so a value that crossed a module boundary can still be resolved back to the
//! declaration that owns it.
//!
//! A `style.when()` declaration also publishes its variant sequence as a
//! [`StoredVariantRecord`], because a module that imports the declaration must expand
//! the sequence at each use.
//!
//! Records are plain values: nothing here registers, deduplicates or validates them,
//! and a record must not be used to identify an asset without its
//! [`AssetRecord::identity`] (labels are not unique).

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use super::identity::IdentityId;

/// Author-visible metadata for one asset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMetadata {
    /// Optional literal label the author gave the declaration, such as
    /// `style.ref("trigger")`. Naming input only: identity never depends on it,
    /// so equal labels stay distinct assets.
    pub label: Option<LeanStr>,
    /// Variant sequence of a `style.when()` declaration. `None` for other asset kinds
    /// and for a declaration whose argument is invalid, whose uses contribute nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stored_variant: Option<StoredVariantRecord>,
}

/// The variant sequence that a `style.when()` declaration stores.
///
/// Segments are ordered from the outermost variant to the innermost, as in a utility
/// template. A use applies every segment, so all of them must hold. Text segments
/// are not validated against a design system; the compiler validates them at each use.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredVariantRecord {
    /// Segments in application order. The list is never empty.
    pub segments: Vec<StoredVariantSegment>,
}

/// One segment of a [`StoredVariantRecord`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StoredVariantSegment {
    /// Ordinary variant text that ends with `:`, such as `hover:md:`.
    Prefix(LeanStr),
    /// A named-container query written as `ref.at(query)`.
    Container {
        /// Ref candidates that supply the container name, in candidate order.
        refs: Vec<IdentityId>,
        /// Query condition without the container name. Empty text tests only the name.
        query: LeanStr,
    },
    /// A ref condition written as a ref call or tag with optional `.in()` scopes.
    Ref {
        /// The condition on the referenced element.
        condition: StoredRefCondition,
        /// Independent scope conditions in source order.
        scopes: Vec<StoredRefCondition>,
    },
    /// Sequences of which at least one must hold, from a declaration with several
    /// arguments. There are at least two sequences.
    Alternatives(Vec<StoredVariantRecord>),
}

/// A condition on the elements that carry one of several refs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRefCondition {
    /// Ref candidates in candidate order.
    pub refs: Vec<IdentityId>,
    /// Variant text tested on the referenced element. It is empty or ends with `:`.
    pub prefix: LeanStr,
}

impl StoredVariantRecord {
    /// Returns the ref identities that the segments mention, including scopes,
    /// container providers, and the segments of alternatives. An identity can occur
    /// more than once.
    #[must_use]
    pub fn refs(&self) -> Vec<&IdentityId> {
        let mut refs = Vec::new();
        for segment in &self.segments {
            match segment {
                StoredVariantSegment::Prefix(_) => {}
                StoredVariantSegment::Container {
                    refs: providers, ..
                } => {
                    refs.extend(providers);
                }
                StoredVariantSegment::Ref { condition, scopes } => {
                    refs.extend(&condition.refs);
                    refs.extend(scopes.iter().flat_map(|scope| &scope.refs));
                }
                StoredVariantSegment::Alternatives(alternatives) => {
                    refs.extend(alternatives.iter().flat_map(Self::refs));
                }
            }
        }
        refs
    }
}

/// One declared asset, keyed by its declaration-site identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRecord {
    /// Stable key of the asset; the only field that identifies it.
    pub identity: IdentityId,
    /// Optional author-provided label.
    pub metadata: AssetMetadata,
    /// UTF-8 byte offset where the producing call starts in the module's original source.
    pub call_start: u32,
    /// UTF-8 byte offset where the producing call ends, exclusive.
    pub call_end: u32,
}
