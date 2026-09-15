//! Generated identifier families and the CSS namespaces they share.
//!
//! One family is one kind of name the compiler allocates: a utility class, a ref
//! marker, an animation name, and so on. Identity is not the same thing as a CSS
//! namespace, because several families can write into one namespace when the default
//! generators reuse a single string per declaration.
//!
//! [`IdentifierPool`] groups the families whose names must stay unique among
//! themselves, because CSS resolves them in one namespace. Validation compares a name
//! to the identity that produced it, so one declaration that appears in two pools is
//! not a conflict and only distinct identities sharing a spelling are reported.

/// One family of generated names.
///
/// The variant decides which generator a host may override for that family, and which
/// pools the resulting name must be checked in. Serialized spelling is used only in
/// reports, never as name content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdentifierKind {
    /// A generated utility class, written to the class attribute and class selectors.
    UtilityClass,
    /// A ref marker class, written where a ref is attached.
    RefMarker,
    /// A scope marker class, written where a scope is attached.
    ScopeMarker,
    /// An `anchor-name` custom-ident, which the element being anchored is named by.
    AnchorName,
    /// A `container-name` custom-ident, used by `@container` queries.
    ContainerName,
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName,
    /// A `view-transition-name` custom-ident.
    ViewTransitionName,
    /// A `view-transition-class` custom-ident.
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
            Self::ScopeMarker => "scope-marker",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }

    /// The CSS namespace this family writes into.
    pub const fn pool(self) -> IdentifierPool {
        match self {
            Self::UtilityClass | Self::RefMarker | Self::ScopeMarker => IdentifierPool::ClassToken,
            Self::AnchorName => IdentifierPool::AnchorName,
            Self::ContainerName => IdentifierPool::ContainerName,
            Self::KeyframesName => IdentifierPool::KeyframesName,
            Self::ViewTransitionName => IdentifierPool::ViewTransitionName,
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IdentifierPool {
    /// Class tokens and class selectors: utility classes and attachment markers.
    ClassToken,
    /// `anchor-name`, `position-anchor`, and `anchor-scope`.
    AnchorName,
    /// `container-name` and `@container` names.
    ContainerName,
    /// `@keyframes` names and `animation-name` values.
    KeyframesName,
    /// `view-transition-name` values. Runtime-derived keyed names are not allocated here.
    ViewTransitionName,
    /// `view-transition-class` values.
    ViewTransitionClass,
    /// Everything in the `--*` namespace: declarations, references, and registrations.
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
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
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
            IdentifierKind::ScopeMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::CustomProperty,
            IdentifierKind::LayerName,
        ];
        let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), kinds.len());
    }
}
