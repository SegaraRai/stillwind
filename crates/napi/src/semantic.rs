//! Typed Node transport for semantic identities. Domain crates do not depend on
//! napi-rs.
//!
//! Every type here mirrors a [`stillwind_semantic`] type with the JSON shape the
//! JavaScript host already consumes. Conversions validate what the domain type would
//! reject through their checked constructors: inconsistent candidate kinds or a ref
//! alternative carrying a non-ref identity or a non-scope value. Missing scopes mean an
//! unscoped ref and are accepted.
//!
//! Export and metadata collections cross the boundary as arrays of named entries rather
//! than as objects, so an export literally named `__proto__` survives the round trip.

use napi::Error;
use napi_derive::napi;
use serde::Serialize;

use stillwind_semantic::{
    AssetKind, AssetRecord, Candidates, ExportedValue, IdentityId, ImportBinding, ModuleSpecifier,
    ModuleSummary, ReExport, RefAlternative, SymbolicValue, TypeFact,
};

/// Asset kinds as the host names them.
#[napi(string_enum = "kebab-case")]
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAssetKind {
    /// A `style(...)` call's payload.
    Style,
    /// A `style.scope()` declaration.
    Scope,
    /// A `style.ref()` declaration.
    Ref,
    /// A `style.animation()` declaration.
    Animation,
    /// A `style.viewTransition()` declaration.
    ViewTransition,
}

impl From<NativeAssetKind> for AssetKind {
    fn from(value: NativeAssetKind) -> Self {
        match value {
            NativeAssetKind::Style => Self::Style,
            NativeAssetKind::Scope => Self::Scope,
            NativeAssetKind::Ref => Self::Ref,
            NativeAssetKind::Animation => Self::Animation,
            NativeAssetKind::ViewTransition => Self::ViewTransition,
        }
    }
}

impl From<AssetKind> for NativeAssetKind {
    fn from(value: AssetKind) -> Self {
        match value {
            AssetKind::Style => Self::Style,
            AssetKind::Scope => Self::Scope,
            AssetKind::Ref => Self::Ref,
            AssetKind::Animation => Self::Animation,
            AssetKind::ViewTransition => Self::ViewTransition,
        }
    }
}

/// Identity of a declaration site.
#[napi(object)]
#[derive(Serialize)]
pub struct NativeIdentity {
    /// Kind of declaration this identity names.
    pub kind: NativeAssetKind,
    /// Normalized module path of the declaring module.
    #[napi(js_name = "module_path")]
    pub module_path: String,
    /// Byte start of the declaration identity: binding-name span for named assets,
    /// call span for styles.
    pub start: u32,
    /// Exclusive byte end of the declaration identity span.
    pub end: u32,
    /// Lexical binding name, or the empty string for anonymous calls.
    #[napi(js_name = "binding_name")]
    pub binding_name: String,
}

impl From<NativeIdentity> for IdentityId {
    fn from(value: NativeIdentity) -> Self {
        Self::new(
            value.kind.into(),
            &value.module_path,
            value.start,
            value.end,
            value.binding_name,
        )
    }
}

impl From<IdentityId> for NativeIdentity {
    fn from(value: IdentityId) -> Self {
        Self {
            kind: value.kind.into(),
            module_path: value.module_path,
            start: value.start,
            end: value.end,
            binding_name: value.binding_name,
        }
    }
}

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

/// User-facing metadata of a produced asset.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeAssetMetadata {
    /// Optional label the user gave the declaration.
    pub label: Option<String>,
}

/// A produced asset as reported in a module summary.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeAssetRecord {
    /// Identity of the declaration site.
    pub identity: NativeIdentity,
    /// Metadata carried with the asset.
    pub metadata: NativeAssetMetadata,
    /// Byte start of the producing call.
    #[napi(js_name = "call_start")]
    pub call_start: u32,
    /// Exclusive byte end of the producing call.
    #[napi(js_name = "call_end")]
    pub call_end: u32,
}

impl From<AssetRecord> for NativeAssetRecord {
    fn from(value: AssetRecord) -> Self {
        Self {
            identity: value.identity.into(),
            metadata: NativeAssetMetadata {
                label: value.metadata.label,
            },
            call_start: value.call_start,
            call_end: value.call_end,
        }
    }
}

/// One import binding of a module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeImportBinding {
    /// Local name in the importing module.
    pub local: String,
    /// Authored module specifier.
    pub source: String,
    /// Name in the exporting module.
    pub imported: String,
}

impl From<ImportBinding> for NativeImportBinding {
    fn from(value: ImportBinding) -> Self {
        Self {
            local: value.local,
            source: value.source,
            imported: value.imported,
        }
    }
}

/// One re-export of a module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeReExport {
    /// Name the module exports.
    pub exported: String,
    /// Authored module specifier.
    pub source: String,
    /// Name in the source module.
    pub imported: String,
}

impl From<ReExport> for NativeReExport {
    fn from(value: ReExport) -> Self {
        Self {
            exported: value.exported,
            source: value.source,
            imported: value.imported,
        }
    }
}

/// An import or export specifier with its location.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleSpecifier {
    /// Authored specifier text.
    pub source: String,
    /// Byte start of the specifier literal.
    pub start: u32,
    /// Byte end of the specifier literal.
    pub end: u32,
}

impl From<ModuleSpecifier> for NativeModuleSpecifier {
    fn from(value: ModuleSpecifier) -> Self {
        Self {
            source: value.source,
            start: value.start,
            end: value.end,
        }
    }
}

/// Span of an expression the host must resolve.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeTypeProbe {
    /// Byte start of the expression.
    pub start: u32,
    /// Byte end of the expression.
    pub end: u32,
}

/// Named entries preserve arbitrary export names, including `__proto__`.
/// One named entry of a summary map.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeExport {
    /// Export name, possibly `__proto__`.
    pub name: String,
    /// Resolved value.
    pub value: NativeExportedValue,
}

/// Imports reachable from one export of a module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeExportImports {
    /// Export name.
    pub name: String,
    /// Imports the export depends on.
    pub imports: Vec<NativeImportBinding>,
}

/// Versioned cross-module summary of one module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleSummary {
    /// Summary schema version; see [`stillwind_semantic::ModuleSummary::SCHEMA`].
    pub schema: u32,
    /// Module this summary describes.
    #[napi(js_name = "module_path")]
    pub module_path: String,
    /// Number of explicit module CSS imports.
    #[napi(js_name = "module_css_imports")]
    pub module_css_imports: u32,
    /// Number of framework macro import statements.
    #[napi(js_name = "macro_imports")]
    pub macro_imports: u32,
    /// Every import or export specifier, in source order.
    #[napi(js_name = "module_specifiers")]
    pub module_specifiers: Vec<NativeModuleSpecifier>,
    /// Expressions the host should resolve.
    #[napi(js_name = "type_probes")]
    pub type_probes: Vec<NativeTypeProbe>,
    /// Import bindings, in source order.
    pub imports: Vec<NativeImportBinding>,
    /// Re-exports, in source order.
    #[napi(js_name = "re_exports")]
    pub re_exports: Vec<NativeReExport>,
    /// Imports referenced by at least one probe.
    #[napi(js_name = "probe_imports")]
    pub probe_imports: Vec<NativeImportBinding>,
    /// Imports each export depends on.
    #[napi(js_name = "export_imports")]
    pub export_imports: Vec<NativeExportImports>,
    /// Assets declared by this module.
    pub assets: Vec<NativeAssetRecord>,
    /// Values this module exports.
    pub exports: Vec<NativeExport>,
}

impl NativeModuleSummary {
    /// Wraps a domain summary, adding the counts that cannot be derived from it.
    pub(crate) fn new(
        summary: ModuleSummary,
        module_css_imports: u32,
        macro_imports: u32,
        type_probes: Vec<NativeTypeProbe>,
    ) -> Self {
        Self {
            schema: summary.schema,
            module_path: summary.module_path,
            module_css_imports,
            macro_imports,
            type_probes,
            module_specifiers: summary
                .module_specifiers
                .into_iter()
                .map(Into::into)
                .collect(),
            imports: summary.imports.into_iter().map(Into::into).collect(),
            re_exports: summary.re_exports.into_iter().map(Into::into).collect(),
            probe_imports: summary.probe_imports.into_iter().map(Into::into).collect(),
            export_imports: summary
                .export_imports
                .into_iter()
                .map(|(name, value)| NativeExportImports {
                    name,
                    imports: value.into_iter().map(Into::into).collect(),
                })
                .collect(),
            assets: summary.assets.into_iter().map(Into::into).collect(),
            exports: summary
                .exports
                .into_iter()
                .map(|(name, value)| NativeExport {
                    name,
                    value: value.into(),
                })
                .collect(),
        }
    }
}
