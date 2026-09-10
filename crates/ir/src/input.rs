//! Host input contract. Original framework sources are opaque text to the core.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use stillwind_source::{FileId, Fingerprint, ProjectionMap, SourceFile, Span};

use crate::AdapterKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLanguage {
    Js,
    Jsx,
    Ts,
    Tsx,
}

impl ScriptLanguage {
    pub fn from_path(path: &str) -> Self {
        match path.rsplit('.').next() {
            Some("ts" | "mts" | "cts") => Self::Ts,
            Some("tsx") => Self::Tsx,
            Some("jsx") => Self::Jsx,
            _ => Self::Js,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConverterIdentity {
    pub name: String,
    pub version: String,
    pub options_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportRealm {
    pub id: u32,
    pub source: Span,
    /// Enclosing script scope whose imports are visible in this realm.
    pub parent: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectionRegion {
    pub source: Span,
    pub import_realm: u32,
    pub attribute_quote: Option<AttributeQuote>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributeQuote {
    Single,
    Double,
}

#[derive(Clone, Debug)]
pub struct FrameworkProjection {
    pub original: Arc<SourceFile>,
    pub map: ProjectionMap,
    pub converter: ConverterIdentity,
    pub analysis_id: FileId,
    pub realms: Vec<ImportRealm>,
    pub regions: Vec<ProjectionRegion>,
}

#[derive(Clone, Debug)]
pub struct SourceInput {
    /// JavaScript/TypeScript presented to the parser, never raw SFC markup.
    pub code: Arc<SourceFile>,
    pub language: ScriptLanguage,
    pub framework: Option<AdapterKind>,
    pub projection: Option<FrameworkProjection>,
    /// Edited-source to upstream-original map. Does not change the edit target.
    pub upstream_map: Option<String>,
    /// Host-allocated stylesheet modules inserted at a verified macro import.
    pub stylesheet_imports: Vec<String>,
    /// Host-allocated JavaScript module exporting this module's CSS value.
    pub module_css_import: Option<String>,
}

impl SourceInput {
    pub fn normal(code: Arc<SourceFile>) -> Self {
        Self {
            language: ScriptLanguage::from_path(code.path()),
            code,
            framework: None,
            projection: None,
            upstream_map: None,
            stylesheet_imports: Vec::new(),
            module_css_import: None,
        }
    }

    pub fn edit_source(&self) -> &Arc<SourceFile> {
        self.projection
            .as_ref()
            .map_or(&self.code, |projection| &projection.original)
    }

    /// Includes source, parser, framework, map, converter settings and realms.
    /// Debug representations contain values only (no addresses or host state).
    pub fn revision(&self) -> Fingerprint {
        Fingerprint::of(format!("{self:?}"))
    }
}
