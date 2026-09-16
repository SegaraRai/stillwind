//! Generated names: families, brands, requests, and allocation.
//!
//! The compiler decides one name per identity, and the type of that name says where the name may
//! be written. [`kind`] holds the vocabulary - the families, their CSS namespaces, and the mode a
//! compilation runs in. [`name`] holds the brand types that clean a name and write it. [`request`]
//! holds the context and the identity a name is allocated from, and [`generator`] holds the host
//! injection point together with one compilation's memo of allocated names.
//!
//! A built-in name carries the family's prefix, which separates compiler names from authored CSS.
//! A host-provided name is written exactly as returned, with no prefix: a host that computes its
//! own names owns that separation too.

mod generator;
mod kind;
mod name;
mod request;

pub use generator::{
    DetachedName, GeneratedName, GeneratorSet, IdentifierContext, IdentifierGenerator, Identifiers,
    RejectedIdentifier, RejectedNames,
};
pub use kind::{IdentifierKind, IdentifierMode, IdentifierPool};
pub use name::{
    AnchorName, ContainerName, CustomPropertyName, IdentifierNameError, KeyframesName, Label,
    LayerName, RefMarker, ScopeMarker, Token, UtilityClass, ViewTransitionClass,
};
pub use request::{
    AnchorNameRequest, ClassContext, ClassName, ConditionContext, ConditionHelperRequest,
    ConditionRole, ContainerNameRequest, DeclarationContext, IdentifierHash, KeyframesNameRequest,
    LayerContext, LayerNameRequest, LocatorContext, NameRequest, RefMarkerName, ScopeMarkerName,
    ViewTransitionClassRequest,
};

#[cfg(test)]
mod tests;
