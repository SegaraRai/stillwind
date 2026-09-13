//! Node module summaries and asset metadata, preserving ordered named entries.
//!
//! [`NativeModuleSummary::new`] converts domain summaries and adds frontend counts.
//! Export names such as `__proto__` remain array entries rather than object keys.

use napi_derive::napi;
use serde::Serialize;

use stillwind_semantic::{AssetRecord, ImportBinding, ModuleSpecifier, ModuleSummary, ReExport};

use super::identity::NativeIdentity;
use super::values::NativeExportedValue;

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
