//! Generated names: families, brands, requests, and allocation.
//!
//! The compiler decides one name per identity, and the type of that name says where the
//! name may be written. A default name carries the prefix of its family, which
//! separates compiler names from authored CSS. A host-provided name is written exactly
//! as returned, without a prefix, so a host that computes its own names must keep them
//! apart from authored CSS itself.
//!
//! Modules:
//!
//! - [`generator`] holds the host injection point and the memo of names that one
//!   compilation allocates.
//! - [`kind`] holds the name families, their CSS namespaces, and the mode of a
//!   compilation.
//! - [`name`] holds the brand types that clean and write a name.
//! - [`preview`] assigns readable display names after normal allocation and validation.
//! - [`request`] holds the context and the identity that a name is allocated from.

mod generator;
mod kind;
mod name;
mod preview;
mod request;

pub use generator::{
    GeneratedName, GeneratorSet, IdentifierContext, IdentifierGenerator, Identifiers,
    RejectedIdentifier, RejectedNames,
};
pub use kind::{IdentifierKind, IdentifierMode, IdentifierPool};
pub use name::{
    AnchorName, ConditionHelperName, ContainerName, IdentifierNameError, KeyframesName, Label,
    LayerName, RefMarker, Token, UtilityClass, ViewTransitionClass, ViewTransitionSlotName,
};
pub use request::{
    AnchorNameRequest, ClassContext, ClassName, ConditionContext, ConditionHelperRequest,
    ConditionRole, ContainerNameRequest, DeclarationContext, IdentifierHash, KeyframesNameRequest,
    LayerContext, LayerNameRequest, LocatorContext, LocatorProjection, NameRequest, RefMarkerName,
    ViewTransitionClassRequest, ViewTransitionSlotRequest,
};

pub(crate) use generator::DetachedName;
pub(crate) use kind::{DEFAULT_HASH_LENGTH, MAXIMUM_HASH_LENGTH, default_prefix};
pub(crate) use name::{escaped_identifier, view_transition_name};
pub(crate) use preview::{PreviewNames, PreviewRequest};
pub(crate) use request::{declaration_fields, key_fields};

#[cfg(test)]
mod tests;
