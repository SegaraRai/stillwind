//! Identifier families and the CSS namespaces they are written into.
//!
//! One family is one kind of name the compiler allocates: a utility class, a ref
//! marker, an keyframes name, and so on. Identity is not the same thing as a CSS
//! namespace, because several families can write into one namespace when the built-in
//! generators reuse a single string per declaration.
//!
//! [`IdentifierPool`] groups the families whose names must stay unique among
//! themselves, including the independent pool for minification barriers. Validation
//! compares a name to the identity that produced it, so one declaration that appears in
//! two pools is not a conflict and only distinct identities sharing a spelling are
//! reported.
//!
//! A prefix (the class, marker, and custom-property prefixes below) marks a name as
//! compiler-owned and is added by the built-in generator only. A host body is written
//! exactly as returned, with no prefix: the prefix separates compiler names from
//! authored CSS, and a host that computes its own names owns that separation.
//!
//! The generation, cleaning, and escaping of a family's names live in
//! [`crate::identifiers`].

use serde::{Deserialize, Serialize};

/// The prefix of a built-in utility class token.
pub(crate) const DEFAULT_CLASS_PREFIX: &str = "sw_";

/// The prefix that marks a built-in ref marker class token as compiler-owned.
pub(crate) const DEFAULT_REF_MARKER_PREFIX: &str = "sw_";

/// The prefix of a built-in ref anchor name, without the value's own leading hyphens.
///
/// The anchor properties take a `<dashed-ident>`, so the two hyphens belong to the
/// value grammar and are added by the family's write methods rather than carried by the
/// name.
pub(crate) const DEFAULT_ANCHOR_PREFIX: &str = "sw-a-";

/// The prefix of a built-in container name.
pub(crate) const DEFAULT_CONTAINER_PREFIX: &str = "sw-c-";

/// The prefix of a built-in keyframes name.
pub(crate) const DEFAULT_KEYFRAMES_PREFIX: &str = "sw-kf-";

/// The prefix of a built-in view-transition class token, which is also the transition's
/// identity.
pub(crate) const DEFAULT_VIEW_TRANSITION_CLASS_PREFIX: &str = "sw-vt-";

/// The prefix of a built-in conditional helper, before the role letter and separator.
///
/// A helper is a custom property, and that family's render method adds the leading
/// hyphens, so the canonical spelling this prefix starts carries none.
pub(crate) const DEFAULT_CONDITION_PREFIX: &str = "sw-if";

/// The prefix of the custom property one guarded view-transition use writes into,
/// without its leading hyphens.
pub(crate) const DEFAULT_VIEW_TRANSITION_SLOT_PREFIX: &str = "sw-vtc-";

/// The prefix of a built-in anonymous layer name.
pub(crate) const DEFAULT_LAYER_PREFIX: &str = "sw-anon-";

/// How many base-36 digits of the request hash a built-in name keeps when a compilation
/// does not configure [`crate::CompilerOptions::hash_length`].
pub(crate) const DEFAULT_HASH_LENGTH: usize = 5;

/// The longest base-36 spelling of one 256-bit digest.
///
/// `36^50` exceeds `2^256` while `36^49` does not, so no digest needs a 51st digit and
/// a longer spelling could only repeat leading zeros.
pub(crate) const MAXIMUM_HASH_LENGTH: usize = 50;

/// When the compilation runs, which decides how a rejected name is handled.
///
/// The mode is an input of the compilation, not of [`crate::CompilerOptions`]: it
/// changes what the compiler generates, and a condition-resolved module specifier must
/// not stand in for it, because the same generated code would then meet both the
/// development and the release implementation of a helper.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IdentifierMode {
    /// A release compilation. A rejected name fails the compilation, so a host build
    /// never ships a name the host expected to control.
    #[default]
    Release,
    /// A development compilation. A rejected name falls back to the built-in name and is
    /// recorded in [`crate::identifiers::RejectedNames`], so a running dev server stays usable while the
    /// host can warn about the generator.
    Development,
}

/// One family of generated names.
///
/// The variant decides which pool the resulting name must be checked in and how a report
/// spells the family. Serialized spelling is used only in reports, never as name content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierKind {
    /// A generated utility class, written to the class attribute and class selectors.
    UtilityClass,
    /// A ref marker class, written where a ref is attached.
    RefMarker,
    /// An `anchor-name` custom-ident, which the element being anchored is named by.
    AnchorName,
    /// A `container-name` custom-ident, used by `@container` queries.
    ContainerName,
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName,
    /// A `view-transition-class` custom-ident, which is also the transition's identity.
    ViewTransitionClass,
    /// A conditional helper custom property, without its leading `--`.
    ConditionHelper,
    /// A support-query name preserving conditional rule boundaries during minification.
    RuleBoundary,
    /// The custom property one guarded view-transition use writes its class token into,
    /// without its leading `--`.
    ViewTransitionSlot,
    /// A generated `@layer` name.
    LayerName,
}

impl IdentifierKind {
    /// Stable lowercase spelling used in symbol records and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UtilityClass => "class",
            Self::RefMarker => "ref-marker",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionClass => "view-transition-class",
            Self::ConditionHelper => "condition-helper",
            Self::RuleBoundary => "rule-boundary",
            Self::ViewTransitionSlot => "view-transition-slot",
            Self::LayerName => "layer",
        }
    }

    /// The CSS namespace this family writes into.
    pub const fn pool(self) -> IdentifierPool {
        match self {
            Self::UtilityClass | Self::RefMarker => IdentifierPool::ClassToken,
            Self::AnchorName => IdentifierPool::AnchorName,
            Self::ContainerName => IdentifierPool::ContainerName,
            Self::KeyframesName => IdentifierPool::KeyframesName,
            Self::ViewTransitionClass => IdentifierPool::ViewTransitionClass,
            Self::ConditionHelper | Self::ViewTransitionSlot => IdentifierPool::CustomProperty,
            Self::RuleBoundary => IdentifierPool::RuleBoundary,
            Self::LayerName => IdentifierPool::LayerName,
        }
    }
}

/// One collision domain whose names must be distinct across the document.
///
/// Names in different pools do not interact, so the same spelling may appear in both;
/// within one pool a spelling may only belong to one identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierPool {
    /// Class tokens and class selectors: utility classes and attachment markers.
    ClassToken,
    /// `anchor-name`, `position-anchor`, and `anchor-scope`.
    AnchorName,
    /// `container-name` and `@container` names.
    ContainerName,
    /// `@keyframes` names and `animation-name` values.
    KeyframesName,
    /// `view-transition-class` values, which are the transitions' identities.
    ///
    /// A `view-transition-name` is a pairing key rather than an allocated name: it is
    /// composed from a token and a key, and different identities may share one value.
    ViewTransitionClass,
    /// Everything in the custom-property namespace.
    CustomProperty,
    /// Support-query identities that keep conditional rules from merging.
    RuleBoundary,
    /// `@layer` names in the document and in CSS carried by modules.
    LayerName,
}

impl IdentifierPool {
    /// Stable lowercase spelling used in diagnostics and reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassToken => "class",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::RuleBoundary => "rule-boundary",
            Self::LayerName => "layer",
        }
    }
}

/// The prefix the built-in generator puts in front of a family's name.
///
/// A host-provided name is written without it: the prefix exists to separate compiler
/// names from authored CSS, and a host that computes its own names owns that separation.
pub(crate) const fn builtin_prefix(kind: IdentifierKind) -> &'static str {
    match kind {
        IdentifierKind::UtilityClass => DEFAULT_CLASS_PREFIX,
        IdentifierKind::RefMarker => DEFAULT_REF_MARKER_PREFIX,
        IdentifierKind::AnchorName => DEFAULT_ANCHOR_PREFIX,
        IdentifierKind::ContainerName => DEFAULT_CONTAINER_PREFIX,
        IdentifierKind::KeyframesName => DEFAULT_KEYFRAMES_PREFIX,
        IdentifierKind::ViewTransitionClass => DEFAULT_VIEW_TRANSITION_CLASS_PREFIX,
        IdentifierKind::ConditionHelper => DEFAULT_CONDITION_PREFIX,
        IdentifierKind::RuleBoundary => "",
        IdentifierKind::ViewTransitionSlot => DEFAULT_VIEW_TRANSITION_SLOT_PREFIX,
        IdentifierKind::LayerName => DEFAULT_LAYER_PREFIX,
    }
}

/// The character a built-in name puts between an authored label and the digest.
///
/// A name separates the parts of its own prefix: the class tokens whose prefix ends in an
/// underscore keep `_`, and every dash-separated family keeps `-`.
pub(crate) const fn builtin_label_separator(kind: IdentifierKind) -> char {
    match kind {
        IdentifierKind::UtilityClass | IdentifierKind::RefMarker => '_',
        _ => '-',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_map_to_the_namespace_they_are_written_in() {
        assert_eq!(
            IdentifierKind::UtilityClass.pool(),
            IdentifierPool::ClassToken
        );
        assert_eq!(IdentifierKind::RefMarker.pool(), IdentifierPool::ClassToken);
        assert_eq!(
            IdentifierKind::AnchorName.pool(),
            IdentifierPool::AnchorName
        );
        assert_eq!(
            IdentifierKind::ViewTransitionClass.pool(),
            IdentifierPool::ViewTransitionClass
        );
        assert_eq!(
            IdentifierKind::KeyframesName.pool(),
            IdentifierPool::KeyframesName
        );
        assert_eq!(
            IdentifierKind::ConditionHelper.pool(),
            IdentifierPool::CustomProperty
        );
        assert_eq!(
            IdentifierKind::ViewTransitionSlot.pool(),
            IdentifierPool::CustomProperty
        );
        assert_eq!(
            IdentifierKind::RuleBoundary.pool(),
            IdentifierPool::RuleBoundary
        );
    }

    #[test]
    fn report_spellings_are_distinct_per_family() {
        let kinds = [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::ConditionHelper,
            IdentifierKind::RuleBoundary,
            IdentifierKind::ViewTransitionSlot,
            IdentifierKind::LayerName,
        ];
        let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), kinds.len());
    }

    #[test]
    fn only_families_the_builtin_generator_owns_carry_a_prefix() {
        assert!(builtin_prefix(IdentifierKind::RuleBoundary).is_empty());
        for kind in [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::ConditionHelper,
            IdentifierKind::ViewTransitionSlot,
            IdentifierKind::LayerName,
        ] {
            assert!(!builtin_prefix(kind).is_empty(), "{kind:?}");
        }
    }
}
