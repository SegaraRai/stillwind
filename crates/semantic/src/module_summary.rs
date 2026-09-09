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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub schema: u32,
    pub module_path: String,
    pub imports: Vec<ImportBinding>,
    pub re_exports: Vec<ReExport>,
    pub assets: Vec<AssetRecord>,
    pub exports: BTreeMap<String, ExportedValue>,
}

impl ModuleSummary {
    pub const SCHEMA: u32 = 1;

    pub fn new(module_path: &str) -> Self {
        Self {
            schema: Self::SCHEMA,
            module_path: normalized_module_path(module_path),
            imports: Vec::new(),
            re_exports: Vec::new(),
            assets: Vec::new(),
            exports: BTreeMap::new(),
        }
    }
}
