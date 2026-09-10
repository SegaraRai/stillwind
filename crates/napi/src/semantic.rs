//! Typed Node transport for semantic identities. Domain crates do not depend on napi-rs.
use napi::Error;
use napi_derive::napi;
use serde::Serialize;
use stillwind_semantic::{
    AssetKind, AssetRecord, Candidates, ExportedValue, IdentityId, ImportBinding, ModuleSpecifier,
    ModuleSummary, ReExport, RefAlternative, SymbolicValue, TypeFact,
};

#[napi(string_enum = "kebab-case")]
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAssetKind {
    Scope,
    Ref,
    Animation,
    ViewTransition,
}

impl From<NativeAssetKind> for AssetKind {
    fn from(value: NativeAssetKind) -> Self {
        match value {
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
            AssetKind::Scope => Self::Scope,
            AssetKind::Ref => Self::Ref,
            AssetKind::Animation => Self::Animation,
            AssetKind::ViewTransition => Self::ViewTransition,
        }
    }
}

#[napi(object)]
#[derive(Serialize)]
pub struct NativeIdentity {
    pub kind: NativeAssetKind,
    #[napi(js_name = "module_path")]
    pub module_path: String,
    pub start: u32,
    pub end: u32,
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

#[napi(discriminant = "kind", discriminant_case = "camelCase")]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NativeCandidates {
    Universe,
    Finite { values: Vec<NativeIdentity> },
}

#[napi(object)]
#[derive(Serialize)]
pub struct NativeSymbolicValue {
    pub kind: NativeAssetKind,
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

#[napi(object, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeRefAlternative {
    pub reference: NativeIdentity,
    #[napi(js_name = "permitted_scopes")]
    pub permitted_scopes: Option<NativeSymbolicValue>,
}

#[napi(discriminant = "kind", discriminant_case = "kebab-case")]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NativeExportedValue {
    Scopes { value: NativeSymbolicValue },
    Refs { value: Vec<NativeRefAlternative> },
    Animations { value: NativeSymbolicValue },
    ViewTransitions { value: NativeSymbolicValue },
}

impl TryFrom<NativeExportedValue> for ExportedValue {
    type Error = Error;
    fn try_from(input: NativeExportedValue) -> napi::Result<Self> {
        let (value, expected) = match input {
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
            Self::Scopes(value) | Self::Animations(value) | Self::ViewTransitions(value) => {
                value.kind
            }
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

#[napi(object)]
pub struct NativeTypeFact {
    pub start: u32,
    pub end: u32,
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

#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeAssetMetadata {
    pub label: Option<String>,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeAssetRecord {
    pub identity: NativeIdentity,
    pub metadata: NativeAssetMetadata,
    #[napi(js_name = "call_start")]
    pub call_start: u32,
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

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeImportBinding {
    pub local: String,
    pub source: String,
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

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeReExport {
    pub exported: String,
    pub source: String,
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

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleSpecifier {
    pub source: String,
    pub start: u32,
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

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeTypeProbe {
    pub start: u32,
    pub end: u32,
}

/// Named entries preserve arbitrary export names, including `__proto__`.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeExport {
    pub name: String,
    pub value: NativeExportedValue,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeExportImports {
    pub name: String,
    pub imports: Vec<NativeImportBinding>,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleSummary {
    pub schema: u32,
    #[napi(js_name = "module_path")]
    pub module_path: String,
    #[napi(js_name = "module_css_imports")]
    pub module_css_imports: u32,
    #[napi(js_name = "macro_imports")]
    pub macro_imports: u32,
    #[napi(js_name = "module_specifiers")]
    pub module_specifiers: Vec<NativeModuleSpecifier>,
    #[napi(js_name = "type_probes")]
    pub type_probes: Vec<NativeTypeProbe>,
    pub imports: Vec<NativeImportBinding>,
    #[napi(js_name = "re_exports")]
    pub re_exports: Vec<NativeReExport>,
    #[napi(js_name = "probe_imports")]
    pub probe_imports: Vec<NativeImportBinding>,
    #[napi(js_name = "export_imports")]
    pub export_imports: Vec<NativeExportImports>,
    pub assets: Vec<NativeAssetRecord>,
    pub exports: Vec<NativeExport>,
}

impl NativeModuleSummary {
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
