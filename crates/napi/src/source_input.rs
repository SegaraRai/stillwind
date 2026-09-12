//! Host transport for normal and projected source buffers.
//!
//! The JavaScript side describes a module with two texts: the buffer a parser sees,
//! and, when a converter produced it, the original buffer the user edits. Offsets
//! arriving from JavaScript are UTF-16 code units, so projection records are translated
//! through [`CoordinateIndex`] before they become spans; a surrogate interior is
//! rejected rather than rounded.
//!
//! The conversion also enforces the pairing: a projected input must carry its map,
//! converter identity, language, and framework, and a plain input must not carry
//! projection metadata at all. This validation applies to transport conversion;
//! domain records remain publicly constructible and need validation at use sites.

use std::sync::Arc;

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use stillwind_ir::{
    AdapterKind, AttributeQuote, ConverterIdentity, FrameworkProjection, ImportRealm,
    ProjectionRegion, ScriptLanguage, SourceInput,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{
    CoordinateIndex, FileId, ProjectionMap, SourceFile, Span, Utf16Offset, VolarMapping,
};

use crate::semantic::NativeTypeFact;

/// Script dialect requested by the host.
#[napi(string_enum = "lowercase")]
pub enum NativeScriptLanguage {
    /// Plain JavaScript.
    Js,
    /// JavaScript with JSX syntax.
    Jsx,
    /// TypeScript without JSX.
    Ts,
    /// TypeScript with JSX syntax.
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

/// Framework adapter requested by the host.
#[napi(string_enum)]
pub enum NativeFramework {
    /// React.
    React,
    /// Preact.
    Preact,
    /// Solid.
    Solid,
    /// Vue.
    Vue,
    /// Svelte.
    Svelte,
    /// Astro.
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

/// Quote character that delimited a converted attribute value.
#[napi(string_enum = "lowercase")]
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeAttributeQuote {
    /// Single-quoted attribute.
    Single,
    /// Double-quoted attribute.
    Double,
}

/// Plain JavaScript source input. Optional vectors default to empty in native code.
#[napi(object, object_to_js = false)]
pub struct NativeProjectSource {
    /// Module path as the host knows it; used for identity, never for reading.
    pub path: String,
    /// Text the parser sees, and the buffer edits are applied to when there is no
    /// projection.
    pub source: String,
    /// Facts the host produced for this module, empty when it has none.
    #[napi(js_name = "type_facts")]
    pub type_facts: Option<Vec<NativeTypeFact>>,
    /// Explicit dialect; defaults to the extension of [`Self::path`].
    pub language: Option<NativeScriptLanguage>,
    /// Adapter to use; absent for plain JavaScript/TypeScript.
    pub framework: Option<NativeFramework>,
    /// Source map from `source` back to the file it was generated from.
    pub map: Option<String>,
    /// Original framework source, present only for projected inputs.
    #[napi(js_name = "fw_code")]
    pub fw_code: Option<String>,
    /// Mapping between [`Self::fw_code`] and [`Self::source`].
    #[napi(js_name = "fw2js_map")]
    pub fw2js_map: Option<NativeProjectionMap>,
    /// Source map from [`Self::fw_code`] to its own upstream file.
    #[napi(js_name = "fw_map")]
    pub fw_map: Option<String>,
    /// Converter that produced [`Self::source`], part of the input revision.
    pub converter: Option<NativeConverterIdentity>,
    /// Import scopes of [`Self::fw_code`].
    #[napi(js_name = "import_realms")]
    pub import_realms: Option<Vec<HostRealm>>,
    /// Converted regions of [`Self::fw_code`].
    pub regions: Option<Vec<HostRegion>>,
    #[napi(js_name = "stylesheet_imports")]
    /// Host-allocated stylesheet modules to insert at the macro import.
    pub stylesheet_imports: Option<Vec<String>>,
    #[napi(js_name = "module_css_import")]
    /// Host-allocated module that exports this module's CSS value.
    pub module_css_import: Option<String>,
}

/// Projection map in either supported encoding.
#[napi(
    discriminant = "kind",
    discriminant_case = "camelCase",
    object_to_js = false
)]
pub enum NativeProjectionMap {
    /// Standard source-map JSON.
    V3 {
        /// Source-map JSON text.
        json: String,
        /// Source name identifying the original file within the source map.
        source: String,
    },
    /// Volar/Vue language-tool mappings.
    Volar {
        /// Mapping entries, in generated order.
        mappings: Vec<NativeVolarMapping>,
    },
}

/// One Volar mapping entry; the four arrays are parallel.
#[napi(object, object_to_js = false)]
pub struct NativeVolarMapping {
    /// UTF-16 offsets in the original source.
    pub source_offsets: Vec<u32>,
    /// UTF-16 offsets in the generated source.
    pub generated_offsets: Vec<u32>,
    /// Length of each mapping in the original source.
    pub lengths: Vec<u32>,
    /// Optional per-mapping generated lengths, when they differ.
    pub generated_lengths: Option<Vec<u32>>,
}

/// Converter that produced a projected input.
#[napi(object, object_to_js = false)]
pub struct NativeConverterIdentity {
    /// Tool name.
    pub name: String,
    /// Tool version.
    pub version: String,
    /// Fingerprint of the converter settings that produced the mapping.
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
                        .map(|mapping| VolarMapping {
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

/// Domain-side projection record deserialized from the host transport.
#[derive(Deserialize)]
pub struct ProjectSource {
    /// Module path as the host knows it.
    pub path: String,
    /// Text the parser sees.
    pub source: String,
    /// Facts the host produced for this module.
    #[serde(default)]
    pub type_facts: Vec<TypeFact>,
    /// Explicit dialect, or `None` to derive it from [`Self::path`].
    pub language: Option<ScriptLanguage>,
    /// Adapter to use, or `None` for plain JavaScript/TypeScript.
    pub framework: Option<AdapterKind>,
    /// Source map from `source` to its upstream file.
    pub map: Option<String>,
    /// Original framework source for a projected input.
    pub fw_code: Option<String>,
    /// Mapping between [`Self::fw_code`] and [`Self::source`].
    pub fw2js_map: Option<ProjectionMap>,
    /// Source map from [`Self::fw_code`] to its own upstream file.
    pub fw_map: Option<String>,
    /// Converter identity for a projected input.
    pub converter: Option<ConverterIdentity>,
    /// Import scopes of [`Self::fw_code`].
    #[serde(default)]
    pub import_realms: Vec<HostRealm>,
    /// Converted regions of [`Self::fw_code`].
    #[serde(default)]
    pub regions: Vec<HostRegion>,
    /// Host-allocated stylesheet modules.
    #[serde(default)]
    pub stylesheet_imports: Vec<String>,
    /// Host-allocated module that exports this module's CSS value.
    pub module_css_import: Option<String>,
}

/// Source offsets in the host transport are UTF-16; native spans are bytes.
#[napi(object, object_to_js = false)]
#[derive(Deserialize)]
pub struct HostRealm {
    /// Host-chosen realm identifier.
    pub id: u32,
    /// UTF-16 start of the realm in the original source.
    pub start: u32,
    /// UTF-16 end of the realm in the original source.
    pub end: u32,
    /// Enclosing realm, or `None` for the file-level script scope.
    pub parent: Option<u32>,
}

/// Converted region of the original source, in UTF-16 offsets.
#[napi(object, object_to_js = false)]
#[derive(Deserialize)]
pub struct HostRegion {
    /// UTF-16 start in the original source.
    pub start: u32,
    /// UTF-16 end in the original source.
    pub end: u32,
    /// Realm whose imports are visible inside this region.
    #[napi(js_name = "import_realm")]
    pub import_realm: u32,
    /// Delimiter of the enclosing attribute, when there is one.
    #[napi(js_name = "attribute_quote")]
    pub attribute_quote: Option<NativeAttributeQuote>,
}

impl ProjectSource {
    /// Builds the analyzer input, translating UTF-16 projection offsets into
    /// byte spans and validating that the projection metadata is complete.
    ///
    /// `original_id` names the editable file, `generated_id` the buffer the
    /// parser sees, and `analysis_id` the file id used for spans created while
    /// analyzing that buffer.
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
