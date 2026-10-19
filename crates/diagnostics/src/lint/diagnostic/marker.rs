//! Candidate forms that cannot add a marker class.

use serde::{Deserialize, Serialize};

/// The reason a candidate that names a marker class cannot add the class.
///
/// A marker class is added to the element as written, so it cannot depend on a
/// condition or carry utility syntax that changes declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum MarkerUseErrorKind {
    /// The candidate has variants, or an enclosing style object applies variants or ref
    /// conditions to it.
    #[error("under variants or ref conditions")]
    Conditional,
    /// The candidate is marked `!important`.
    #[error("with `!`")]
    Important,
    /// The candidate has a leading `-`.
    #[error("with a leading `-`")]
    Negative,
    /// The marker accepts no modifier, or the modifier is not a named modifier.
    #[error("with this modifier")]
    Modifier,
    /// The candidate is written where no element class list receives it, such as in
    /// `@apply` or in a document-level style.
    #[error("outside the class list of an element")]
    Placement,
}
