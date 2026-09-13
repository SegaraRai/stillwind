//! Checked symbolic values, correlated ref alternatives, and host type facts.
//!
//! Conversions reject inconsistent candidate kinds and invalid ref/scope pairs.
//! Absent scope constraints retain their domain meaning as unconstrained refs.

use napi::Error;
use napi_derive::napi;
use serde::Serialize;

use stillwind_semantic::{
    AssetKind, Candidates, ExportedValue, RefAlternative, SymbolicValue, TypeFact,
};

use super::identity::{NativeAssetKind, NativeIdentity};

/// The candidate set of a symbolic value.
#[napi(discriminant = "kind", discriminant_case = "camelCase")]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NativeCandidates {
    /// Every identity of the kind is reachable.
    Universe,
    /// Only these identities are reachable.
    Finite {
        /// The reachable identities.
        values: Vec<NativeIdentity>,
    },
}

/// A value whose exact runtime identity may not be known statically.
#[napi(object)]
#[derive(Serialize)]
pub struct NativeSymbolicValue {
    /// Asset kind the value can hold.
    pub kind: NativeAssetKind,
    /// Which identities of that kind are reachable.
    pub candidates: NativeCandidates,
}

impl TryFrom<NativeSymbolicValue> for SymbolicValue {
    type Error = Error;
    fn try_from(value: NativeSymbolicValue) -> napi::Result<Self> {
        let kind = value.kind.into();
        match value.candidates {
            NativeCandidates::Universe => Ok(Self::universe(kind)),
            NativeCandidates::Finite { values } => {
                Self::finite(kind, values.into_iter().map(Into::into))
                    .map_err(|error| Error::from_reason(error.to_string()))
            }
        }
    }
}

impl From<SymbolicValue> for NativeSymbolicValue {
    fn from(value: SymbolicValue) -> Self {
        Self {
            kind: value.kind.into(),
            candidates: match value.candidates {
                Candidates::Universe => NativeCandidates::Universe,
                Candidates::Finite(values) => NativeCandidates::Finite {
                    values: values.into_iter().map(Into::into).collect(),
                },
            },
        }
    }
}

/// One ref candidate together with the scopes it may belong to.
///
/// The pair is a relationship, not a product: see [`stillwind_semantic::RefAlternative`].
#[napi(object, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeRefAlternative {
    /// The ref identity.
    pub reference: NativeIdentity,
    /// Scopes this ref may be bound to, or `None` when unconstrained.
    #[napi(js_name = "permitted_scopes")]
    pub permitted_scopes: Option<NativeSymbolicValue>,
}

/// A value an analyzed module can export.
#[napi(discriminant = "kind", discriminant_case = "kebab-case")]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NativeExportedValue {
    /// Exported `style(...)` payload.
    Styles {
        /// Reachable style identities.
        value: NativeSymbolicValue,
    },
    /// Exported scope.
    Scopes {
        /// Reachable scope identities.
        value: NativeSymbolicValue,
    },
    /// Exported ref, kept as alternatives so correlation is preserved.
    Refs {
        /// The correlated alternatives.
        value: Vec<NativeRefAlternative>,
    },
    /// Exported animation.
    Animations {
        /// Reachable animation identities.
        value: NativeSymbolicValue,
    },
    /// Exported view transition.
    ViewTransitions {
        /// Reachable transition identities.
        value: NativeSymbolicValue,
    },
}

impl TryFrom<NativeExportedValue> for ExportedValue {
    type Error = Error;

    fn try_from(input: NativeExportedValue) -> napi::Result<Self> {
        let (value, expected) = match input {
            NativeExportedValue::Styles { value } => {
                (Self::Styles(value.try_into()?), AssetKind::Style)
            }
            NativeExportedValue::Scopes { value } => {
                (Self::Scopes(value.try_into()?), AssetKind::Scope)
            }
            NativeExportedValue::Animations { value } => {
                (Self::Animations(value.try_into()?), AssetKind::Animation)
            }
            NativeExportedValue::ViewTransitions { value } => (
                Self::ViewTransitions(value.try_into()?),
                AssetKind::ViewTransition,
            ),
            NativeExportedValue::Refs { value } => {
                return Ok(Self::Refs(
                    value
                        .into_iter()
                        .map(|item| {
                            RefAlternative::new(
                                item.reference.into(),
                                item.permitted_scopes.map(TryInto::try_into).transpose()?,
                            )
                            .map_err(|error| Error::from_reason(error.to_string()))
                        })
                        .collect::<napi::Result<_>>()?,
                ));
            }
        };
        let actual = match &value {
            Self::Styles(value)
            | Self::Scopes(value)
            | Self::Animations(value)
            | Self::ViewTransitions(value) => value.kind,
            Self::Refs(_) => AssetKind::Ref,
        };
        if actual != expected {
            return Err(Error::from_reason(
                "Exported value has a mismatched asset kind",
            ));
        }
        Ok(value)
    }
}

impl From<ExportedValue> for NativeExportedValue {
    fn from(input: ExportedValue) -> Self {
        match input {
            ExportedValue::Styles(value) => Self::Styles {
                value: value.into(),
            },
            ExportedValue::Scopes(value) => Self::Scopes {
                value: value.into(),
            },
            ExportedValue::Animations(value) => Self::Animations {
                value: value.into(),
            },
            ExportedValue::ViewTransitions(value) => Self::ViewTransitions {
                value: value.into(),
            },
            ExportedValue::Refs(value) => Self::Refs {
                value: value
                    .into_iter()
                    .map(|item| NativeRefAlternative {
                        reference: item.reference.into(),
                        permitted_scopes: item.permitted_scopes.map(Into::into),
                    })
                    .collect(),
            },
        }
    }
}

/// One expression span and the type the host resolved for it.
#[napi(object)]
pub struct NativeTypeFact {
    /// Byte start of the expression in the original source.
    pub start: u32,
    /// Byte end of the expression in the original source.
    pub end: u32,
    /// The resolved value.
    pub value: NativeExportedValue,
}

impl TryFrom<NativeTypeFact> for TypeFact {
    type Error = Error;
    fn try_from(value: NativeTypeFact) -> napi::Result<Self> {
        Ok(Self {
            start: value.start,
            end: value.end,
            value: value.value.try_into()?,
        })
    }
}
