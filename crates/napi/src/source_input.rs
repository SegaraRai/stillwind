//! Host transport for normal and projected source buffers.

use std::sync::Arc;

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use crate::semantic::NativeTypeFact;
use stillwind_ir::{
    AdapterKind, AttributeQuote, ConverterIdentity, FrameworkProjection, ImportRealm,
    ProjectionRegion, ScriptLanguage, SourceInput,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{CoordinateIndex, FileId, ProjectionMap, SourceFile, Span, Utf16Offset};

#[napi(string_enum = "lowercase")]
pub enum NativeScriptLanguage {
    Js,
    Jsx,
    Ts,
    Tsx,
}
impl From<NativeScriptLanguage> for ScriptLanguage {
    fn from(value: NativeScriptLanguage) -> Self {
        match value {
            NativeScriptLanguage::Js => Self::Js,
            NativeScriptLanguage::Jsx => Self::Jsx,
            NativeScriptLanguage::Ts => Self::Ts,
            NativeScriptLanguage::Tsx => Self::Tsx,
        }
    }
}

#[napi(string_enum)]
pub enum NativeFramework {
    React,
    Preact,
    Solid,
    Vue,
    Svelte,
    Astro,
}
impl From<NativeFramework> for AdapterKind {
    fn from(value: NativeFramework) -> Self {
        match value {
            NativeFramework::React => Self::React,
            NativeFramework::Preact => Self::Preact,
            NativeFramework::Solid => Self::Solid,
            NativeFramework::Vue => Self::Vue,
            NativeFramework::Svelte => Self::Svelte,
            NativeFramework::Astro => Self::Astro,
        }
    }
}

#[napi(string_enum = "lowercase")]
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeAttributeQuote {
    Single,
    Double,
}

/// Plain JavaScript source input. Optional vectors default to empty in native code.
#[napi(object, object_to_js = false)]
pub struct NativeProjectSource {
    pub path: String,
    pub source: String,
    #[napi(js_name = "type_facts")]
    pub type_facts: Option<Vec<NativeTypeFact>>,
    pub language: Option<NativeScriptLanguage>,
    pub framework: Option<NativeFramework>,
    pub map: Option<String>,
    #[napi(js_name = "fw_code")]
    pub fw_code: Option<String>,
    #[napi(js_name = "fw2js_map")]
    pub fw2js_map: Option<NativeProjectionMap>,
    #[napi(js_name = "fw_map")]
    pub fw_map: Option<String>,
    pub converter: Option<NativeConverterIdentity>,
    #[napi(js_name = "import_realms")]
    pub import_realms: Option<Vec<HostRealm>>,
    pub regions: Option<Vec<HostRegion>>,
    #[napi(js_name = "stylesheet_imports")]
    pub stylesheet_imports: Option<Vec<String>>,
    #[napi(js_name = "module_css_import")]
    pub module_css_import: Option<String>,
}

#[napi(
    discriminant = "kind",
    discriminant_case = "camelCase",
    object_to_js = false
)]
pub enum NativeProjectionMap {
    V3 { json: String, source: String },
    Volar { mappings: Vec<NativeVolarMapping> },
}

#[napi(object, object_to_js = false)]
pub struct NativeVolarMapping {
    pub source_offsets: Vec<u32>,
    pub generated_offsets: Vec<u32>,
    pub lengths: Vec<u32>,
    pub generated_lengths: Option<Vec<u32>>,
}

#[napi(object, object_to_js = false)]
pub struct NativeConverterIdentity {
    pub name: String,
    pub version: String,
    #[napi(js_name = "options_fingerprint")]
    pub options_fingerprint: String,
}

impl TryFrom<NativeProjectSource> for ProjectSource {
    type Error = Error;
    fn try_from(value: NativeProjectSource) -> napi::Result<Self> {
        let language = value.language.map(Into::into);
        let framework = value.framework.map(Into::into);
        Ok(Self {
            path: value.path,
            source: value.source,
            type_facts: value
                .type_facts
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<napi::Result<_>>()?,
            language,
            framework,
            map: value.map,
            fw_code: value.fw_code,
            fw_map: value.fw_map,
            fw2js_map: value.fw2js_map.map(|map| match map {
                NativeProjectionMap::V3 { json, source } => ProjectionMap::V3 { json, source },
                NativeProjectionMap::Volar { mappings } => ProjectionMap::Volar {
                    mappings: mappings
                        .into_iter()
                        .map(|mapping| stillwind_source::VolarMapping {
                            source_offsets: mapping.source_offsets,
                            generated_offsets: mapping.generated_offsets,
                            lengths: mapping.lengths,
                            generated_lengths: mapping.generated_lengths,
                        })
                        .collect(),
                },
            }),
            converter: value.converter.map(|converter| ConverterIdentity {
                name: converter.name,
                version: converter.version,
                options_fingerprint: converter.options_fingerprint,
            }),
            import_realms: value.import_realms.unwrap_or_default(),
            regions: value.regions.unwrap_or_default(),
            stylesheet_imports: value.stylesheet_imports.unwrap_or_default(),
            module_css_import: value.module_css_import,
        })
    }
}

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
    pub module_css_import: Option<String>,
}

/// Source offsets in the host transport are UTF-16; native spans are bytes.
#[napi(object, object_to_js = false)]
#[derive(Deserialize)]
pub struct HostRealm {
    pub id: u32,
    pub start: u32,
    pub end: u32,
    pub parent: Option<u32>,
}

#[napi(object, object_to_js = false)]
#[derive(Deserialize)]
pub struct HostRegion {
    pub start: u32,
    pub end: u32,
    #[napi(js_name = "import_realm")]
    pub import_realm: u32,
    #[napi(js_name = "attribute_quote")]
    pub attribute_quote: Option<NativeAttributeQuote>,
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
                        attribute_quote: region.attribute_quote.as_ref().map(|quote| match quote {
                            NativeAttributeQuote::Single => AttributeQuote::Single,
                            NativeAttributeQuote::Double => AttributeQuote::Double,
                        }),
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
            module_css_import: self.module_css_import.clone(),
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
