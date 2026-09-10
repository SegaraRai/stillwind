use super::assets::AssetRecord;
use super::symbolic::ExportedValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use stillwind_source::normalized_module_path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportBinding {
    pub local: String,
    pub source: String,
    pub imported: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReExport {
    pub exported: String,
    pub source: String,
    pub imported: String,
}

/// Exact string-literal span for host-authoritative shadow module resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSpecifier {
    pub source: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSummary {
    #[serde(default)]
    pub module_specifiers: Vec<ModuleSpecifier>,
    pub schema: u32,
    pub module_path: String,
    pub imports: Vec<ImportBinding>,
    pub re_exports: Vec<ReExport>,
    pub assets: Vec<AssetRecord>,
    pub exports: BTreeMap<String, ExportedValue>,
    /// Import bindings reachable from type probes through local declarations.
    #[serde(default)]
    pub probe_imports: Vec<ImportBinding>,
    /// Import bindings needed by each explicitly exported local variable.
    #[serde(default)]
    pub export_imports: BTreeMap<String, Vec<ImportBinding>>,
}

impl ModuleSummary {
    pub const SCHEMA: u32 = 1;

    pub fn new(module_path: &str) -> Self {
        Self {
            module_specifiers: Vec::new(),
            schema: Self::SCHEMA,
            module_path: normalized_module_path(module_path),
            imports: Vec::new(),
            re_exports: Vec::new(),
            assets: Vec::new(),
            exports: BTreeMap::new(),
            probe_imports: Vec::new(),
            export_imports: BTreeMap::new(),
        }
    }
}
