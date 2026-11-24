//! Inputs that a `style()` rule group cannot contain.

use serde::{Deserialize, Serialize};

/// The reason a style input cannot be placed in a rule group.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum RuleGroupInputErrorKind {
    /// Inline custom property supplied by an object entry.
    #[error("inline custom properties")]
    CustomProperty,
    /// Runtime class value without a CSS rule.
    #[error("external class values")]
    ExternalClass,
    /// A composed style value.
    #[error("style compositions")]
    Composition,
    /// A ref attachment.
    #[error("ref attachments")]
    RefAttachment,
    /// A dynamic ref attachment.
    #[error("dynamic ref attachments")]
    DynamicRefAttachment,
    /// Forwarded component props.
    #[error("forwarded props")]
    Forward,
}
