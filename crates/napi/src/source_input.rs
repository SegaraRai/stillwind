//! Host transport for normal and projected source buffers.

use std::sync::Arc;

use napi::Error;
use serde::Deserialize;

use stillwind_ir::{
    AdapterKind, AttributeQuote, ConverterIdentity, FrameworkProjection, ImportRealm,
    ProjectionRegion, ScriptLanguage, SourceInput,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{CoordinateIndex, FileId, ProjectionMap, SourceFile, Span, Utf16Offset};

#[derive(Deserialize)]
pub struct ProjectSource {
    pub path: String,
    pub source: String,
    #[serde(default)]
    pub type_facts: Vec<TypeFact>,
    pub language: Option<ScriptLanguage>,
    pub framework: Option<AdapterKind>,
    pub map: Option<String>,
    pub fw_code: Option<String>,
    pub fw2js_map: Option<ProjectionMap>,
    pub fw_map: Option<String>,
    pub converter: Option<ConverterIdentity>,
    #[serde(default)]
    pub import_realms: Vec<HostRealm>,
    #[serde(default)]
    pub regions: Vec<HostRegion>,
    #[serde(default)]
    pub stylesheet_imports: Vec<String>,
}

/// Source offsets in the host transport are UTF-16; native spans are bytes.
#[derive(Deserialize)]
pub struct HostRealm {
    id: u32,
    start: u32,
    end: u32,
    parent: Option<u32>,
}

#[derive(Deserialize)]
pub struct HostRegion {
    start: u32,
    end: u32,
    import_realm: u32,
    attribute_quote: Option<AttributeQuote>,
}

impl ProjectSource {
    pub fn input(
        &self,
        original_id: FileId,
        generated_id: FileId,
        analysis_id: FileId,
    ) -> napi::Result<SourceInput> {
        let original = self.fw_code.as_ref();
        let language = self
            .language
            .unwrap_or_else(|| ScriptLanguage::from_path(&self.path));
        let code = Arc::new(
            SourceFile::new(
                if original.is_some() {
                    generated_id
                } else {
                    original_id
                },
                self.path.as_str(),
                self.source.as_str(),
            )
            .map_err(|error| Error::from_reason(error.to_string()))?,
        );
        let projection = if let Some(original) = original {
            let original = Arc::new(
                SourceFile::new(original_id, self.path.as_str(), original.as_str())
                    .map_err(|error| Error::from_reason(error.to_string()))?,
            );
            let map = self
                .fw2js_map
                .clone()
                .ok_or_else(|| Error::from_reason("fw_code requires fw2js_map"))?;
            let converter = self
                .converter
                .clone()
                .ok_or_else(|| Error::from_reason("fw_code requires converter identity"))?;
            if self.language.is_none() || self.framework.is_none() {
                return Err(Error::from_reason(
                    "projected input requires language and framework",
                ));
            }
            if self.map.is_some() {
                return Err(Error::from_reason(
                    "projected input uses fw_map instead of map",
                ));
            }
            let index = CoordinateIndex::new(&original);
            let span = |start, end| -> napi::Result<Span> {
                let start = index
                    .byte(Utf16Offset(start))
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                let end = index
                    .byte(Utf16Offset(end))
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                let span = Span::new(original_id, start.0, end.0);
                original
                    .slice(span)
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                Ok(span)
            };
            let realms = self
                .import_realms
                .iter()
                .map(|realm| {
                    Ok(ImportRealm {
                        id: realm.id,
                        source: span(realm.start, realm.end)?,
                        parent: realm.parent,
                    })
                })
                .collect::<napi::Result<Vec<_>>>()?;
            let regions = self
                .regions
                .iter()
                .map(|region| {
                    Ok(ProjectionRegion {
                        source: span(region.start, region.end)?,
                        import_realm: region.import_realm,
                        attribute_quote: region.attribute_quote,
                    })
                })
                .collect::<napi::Result<Vec<_>>>()?;
            Some(FrameworkProjection {
                original,
                map,
                converter,
                analysis_id,
                realms,
                regions,
            })
        } else {
            if self.fw2js_map.is_some()
                || self.fw_map.is_some()
                || self.converter.is_some()
                || !self.import_realms.is_empty()
                || !self.regions.is_empty()
            {
                return Err(Error::from_reason("projection metadata requires fw_code"));
            }
            None
        };
        Ok(SourceInput {
            stylesheet_imports: self.stylesheet_imports.clone(),
            code,
            language,
            framework: self.framework,
            upstream_map: if projection.is_some() {
                self.fw_map.clone()
            } else {
                self.map.clone()
            },
            projection,
        })
    }
}
