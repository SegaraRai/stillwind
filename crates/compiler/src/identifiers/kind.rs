//! Identifier families and the CSS namespaces they are written into.
//!
//! One family is one kind of name the compiler allocates: a utility class, a ref
//! marker, an animation name, and so on. Identity is not the same thing as a CSS
//! namespace, because several families can write into one namespace when the built-in
//! generators reuse a single string per declaration.
//!
//! [`IdentifierPool`] groups the families whose names must stay unique among
//! themselves, because CSS resolves them in one namespace. Validation compares a name
//! to the identity that produced it, so one declaration that appears in two pools is
//! not a conflict and only distinct identities sharing a spelling are reported.
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
pub(crate) const DEFAULT_CLASS_PREFIX: &str = "_s_";

/// The prefix that marks a built-in ref marker class token as compiler-owned.
pub(crate) const DEFAULT_REF_MARKER_PREFIX: &str = "s_";

/// The prefix that marks a built-in scope marker class token as compiler-owned.
pub(crate) const DEFAULT_SCOPE_MARKER_PREFIX: &str = "s_";

/// The custom-property prefix of a built-in ref anchor name.
pub(crate) const DEFAULT_ANCHOR_PREFIX: &str = "--sw-r-";

/// The custom-property prefix of a built-in scope container name.
pub(crate) const DEFAULT_CONTAINER_PREFIX: &str = "--sw-s-";

/// The custom-property prefix of a built-in keyframes name.
pub(crate) const DEFAULT_ANIMATION_PREFIX: &str = "--sw-a-";

/// The custom-property prefix of a built-in view-transition class token.
///
/// The token is the transition's identity and its `view-transition-class` value, so it
/// carries the prefix a custom property uses even though it is written as a custom-ident.
pub(crate) const DEFAULT_VIEW_TRANSITION_CLASS_PREFIX: &str = "--sw-v-";

/// The prefix of a built-in conditional helper.
///
/// A helper is a custom property, and that family's render method adds the leading hyphens, so the
/// canonical spelling this prefix starts carries none.
pub(crate) const DEFAULT_CONDITION_PREFIX: &str = "sw-if-";

/// The prefix of a built-in anonymous layer name.
pub(crate) const DEFAULT_LAYER_PREFIX: &str = "sw-anon-";

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
    /// A scope marker class, written where a scope is attached.
    SubtreeMarker,
    /// An `anchor-name` custom-ident, which the element being anchored is named by.
    AnchorName,
    /// A `container-name` custom-ident, used by `@container` queries.
    ContainerName,
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName,
    /// A `view-transition-class` custom-ident, which is also the transition's identity.
    ViewTransitionClass,
    /// A custom property name, without its leading `--`.
    CustomProperty,
    /// A generated `@layer` name.
    LayerName,
}

impl IdentifierKind {
    /// Stable lowercase spelling used in symbol records and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UtilityClass => "class",
            Self::RefMarker => "ref-marker",
            Self::SubtreeMarker => "scope-marker",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }

    /// The CSS namespace this family writes into.
    pub const fn pool(self) -> IdentifierPool {
        match self {
            Self::UtilityClass | Self::RefMarker | Self::SubtreeMarker => {
                IdentifierPool::ClassToken
            }
            Self::AnchorName => IdentifierPool::AnchorName,
            Self::ContainerName => IdentifierPool::ContainerName,
            Self::KeyframesName => IdentifierPool::KeyframesName,
            Self::ViewTransitionClass => IdentifierPool::ViewTransitionClass,
            Self::CustomProperty => IdentifierPool::CustomProperty,
            Self::LayerName => IdentifierPool::LayerName,
        }
    }
}

/// One CSS namespace whose names must be distinct across the document.
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
        IdentifierKind::SubtreeMarker => DEFAULT_SCOPE_MARKER_PREFIX,
        IdentifierKind::AnchorName => DEFAULT_ANCHOR_PREFIX,
        IdentifierKind::ContainerName => DEFAULT_CONTAINER_PREFIX,
        IdentifierKind::KeyframesName => DEFAULT_ANIMATION_PREFIX,
        IdentifierKind::ViewTransitionClass => DEFAULT_VIEW_TRANSITION_CLASS_PREFIX,
        IdentifierKind::CustomProperty => DEFAULT_CONDITION_PREFIX,
        IdentifierKind::LayerName => DEFAULT_LAYER_PREFIX,
    }
}

/// How many hexadecimal digits of the request hash a built-in name keeps.
///
/// A class token stays short because it is written to the class attribute of every element it
/// styles. A helper or a layer name is a custom property or an at-rule prelude, where the extra
/// digits cost nothing and a longer prefix leaves more room for a host's own authored names.
pub(crate) const fn builtin_digest_digits(kind: IdentifierKind) -> usize {
    match kind {
        IdentifierKind::CustomProperty | IdentifierKind::LayerName => 16,
        _ => 8,
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
    }

    #[test]
    fn report_spellings_are_distinct_per_family() {
        let kinds = [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::SubtreeMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::CustomProperty,
            IdentifierKind::LayerName,
        ];
        let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), kinds.len());
    }

    #[test]
    fn only_families_the_builtin_generator_owns_carry_a_prefix() {
        for kind in [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::SubtreeMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::CustomProperty,
            IdentifierKind::LayerName,
        ] {
            assert!(!builtin_prefix(kind).is_empty(), "{kind:?}");
        }
    }
}
