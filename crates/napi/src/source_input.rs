//! Host transport for normal and projected source buffers.
//!
//! The JavaScript side describes a module with two texts: the buffer a parser sees,
//! and, when a converter produced it, the original buffer the user edits. Offsets
//! arriving from JavaScript are UTF-16 code units, so projection records are translated
//! through [`CoordinateIndex`] before they become spans; a surrogate interior is
//! rejected rather than rounded.
//!
//! The conversion also enforces the pairing: a projected input must carry its map,
//! converter identity, language, and component source format. A plain input must not
//! carry projection metadata. This validation applies to transport conversion;
//! domain records remain publicly constructible and need validation at use sites.

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use stillwind_ir::{
    AdapterKind, AttributeQuote, ConverterIdentity, FrameworkProjection, ImportRealm,
    ProjectionRegion, ScriptLanguage, ScriptModuleKind, SourceFormat, SourceInput,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{
    CoordinateIndex, FileId, ModuleId, ProjectionMap, SourceFile, SourceRegistry, SourceUri, Span,
    Utf16Offset, VolarMapping,
};

use crate::semantic::NativeTypeFact;

/// Format of the original editable source supplied by the host.
#[napi(string_enum = "lowercase")]
pub enum NativeSourceFormat {
    /// Plain JavaScript or TypeScript module.
    Script,
    /// JSX or TSX file, including files without JSX expressions.
    Jsx,
    /// Astro component.
    Astro,
    /// Svelte component.
    Svelte,
    /// Vue single-file component.
    Vue,
}

impl From<NativeSourceFormat> for SourceFormat {
    fn from(value: NativeSourceFormat) -> Self {
        match value {
            NativeSourceFormat::Script => Self::Script,
            NativeSourceFormat::Jsx => Self::Jsx,
            NativeSourceFormat::Astro => Self::Astro,
            NativeSourceFormat::Svelte => Self::Svelte,
            NativeSourceFormat::Vue => Self::Vue,
        }
    }
}

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
    /// TypeScript declaration syntax.
    Dts,
}
impl From<NativeScriptLanguage> for ScriptLanguage {
    fn from(value: NativeScriptLanguage) -> Self {
        match value {
            NativeScriptLanguage::Js => Self::Js,
            NativeScriptLanguage::Jsx => Self::Jsx,
            NativeScriptLanguage::Ts => Self::Ts,
            NativeScriptLanguage::Tsx => Self::Tsx,
            NativeScriptLanguage::Dts => Self::Dts,
        }
    }
}

/// Module grammar supplied for the Oxc parser.
#[napi(string_enum = "camelCase")]
pub enum NativeScriptModuleKind {
    /// ECMAScript module.
    Module,
    /// `CommonJS` module.
    CommonJs,
    /// Non-module script.
    Script,
    /// Grammar selected from the buffer's syntax.
    Unambiguous,
}

impl From<NativeScriptModuleKind> for ScriptModuleKind {
    fn from(value: NativeScriptModuleKind) -> Self {
        match value {
            NativeScriptModuleKind::Module => Self::Module,
            NativeScriptModuleKind::CommonJs => Self::CommonJs,
            NativeScriptModuleKind::Script => Self::Script,
            NativeScriptModuleKind::Unambiguous => Self::Unambiguous,
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
    /// Host-supplied absolute URI of the original editable document.
    pub uri: String,
    /// Host-chosen identity of this module.
    #[napi(js_name = "module_id")]
    pub module_id: String,
    /// Text the parser sees, and the buffer edits are applied to when there is no
    /// projection.
    pub source: String,
    /// Facts the host produced for this module, empty when it has none.
    #[napi(js_name = "type_facts")]
    pub type_facts: Option<Vec<NativeTypeFact>>,
    /// Parser dialect supplied by the host.
    pub language: NativeScriptLanguage,
    /// Module grammar, independent of the JavaScript or TypeScript dialect.
    #[napi(js_name = "module_kind")]
    pub module_kind: NativeScriptModuleKind,
    /// Original format. Plain inputs default from language; projected inputs require
    /// an explicit component format independently of the adapter constraint.
    #[napi(js_name = "source_format")]
    pub source_format: Option<NativeSourceFormat>,
    /// Exact source name for generated stylesheet maps.
    #[napi(js_name = "source_map_source")]
    pub source_map_source: Option<String>,
    /// Explicit output adapter constraint; macro imports select it when absent.
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
        let language = value.language.into();
        let module_kind = value.module_kind.into();
        let framework = value.framework.map(Into::into);
        Ok(Self {
            uri: SourceUri::parse(value.uri)
                .map_err(|error| Error::from_reason(error.to_string()))?,
            module_id: ModuleId::from(value.module_id),
            source: value.source,
            type_facts: value
                .type_facts
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<napi::Result<_>>()?,
            language,
            module_kind,
            source_format: value.source_format.map(Into::into),
            source_map_source: value.source_map_source,
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
    /// Host-supplied identity of the original editable document.
    pub uri: SourceUri,
    /// Host-chosen identity of this module.
    pub module_id: ModuleId,
    /// Text the parser sees.
    pub source: String,
    /// Facts the host produced for this module.
    #[serde(default)]
    pub type_facts: Vec<TypeFact>,
    /// Parser dialect supplied by the host.
    pub language: ScriptLanguage,
    /// Module grammar supplied by the host.
    pub module_kind: ScriptModuleKind,
    /// Original format; required for projections, otherwise defaults from language.
    pub source_format: Option<SourceFormat>,
    /// Exact source name for generated stylesheet maps.
    pub source_map_source: Option<String>,
    /// Explicit output adapter constraint; macro imports select it when absent.
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
    /// Builds the analyzer input, translating UTF-16 projection offsets into byte spans
    /// and validating that the projection metadata is complete.
    ///
    /// The source registry issues IDs for the original, generated, and analysis buffers.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source coordinates or incomplete projection
    /// metadata.
    #[allow(clippy::too_many_lines)]
    pub fn input(&self, sources: &mut SourceRegistry) -> napi::Result<SourceInput> {
        let original = self.fw_code.as_ref();
        let language = self.language;
        let module_kind = self.module_kind;
        let source_format = self
            .source_format
            .unwrap_or_else(|| SourceFormat::script(language));
        let code_uri = match original {
            Some(_) => None,
            None => Some(self.uri.clone()),
        };
        let code = sources
            .register(
                code_uri,
                if original.is_some() {
                    None
                } else {
                    self.source_map_source.as_deref().map(Into::into)
                },
                self.source.as_str(),
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let projection = if let Some(original) = original {
            let original = sources
                .register(
                    Some(self.uri.clone()),
                    self.source_map_source.as_deref().map(Into::into),
                    original.as_str(),
                )
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let original_id = original.id();
            let analysis_id = sources
                .reserve_generated_id()
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let map = self
                .fw2js_map
                .clone()
                .ok_or_else(|| Error::from_reason("fw_code requires fw2js_map"))?;
            let converter = self
                .converter
                .clone()
                .ok_or_else(|| Error::from_reason("fw_code requires converter identity"))?;
            if self
                .source_format
                .and_then(SourceFormat::projection_adapter)
                .is_none()
            {
                return Err(Error::from_reason(
                    "projected input requires a component source_format",
                ));
            }
            if self.map.is_some() {
                return Err(Error::from_reason(
                    "projected input uses fw_map instead of map",
                ));
            }
            let index = CoordinateIndex::new(&original);
            let span = |start, end| projection_span(&index, &original, original_id, start, end);
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
            module_id: self.module_id.clone(),
            stylesheet_imports: self.stylesheet_imports.clone(),
            module_css_import: self.module_css_import.clone(),
            code,
            language,
            module_kind,
            source_format,
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

/// Converts one host region from original-source UTF-16 offsets to a validated span.
///
/// # Errors
///
/// Returns an error when either offset is invalid or the resulting span is reversed.
fn projection_span(
    index: &CoordinateIndex,
    original: &SourceFile,
    original_id: FileId,
    start: u32,
    end: u32,
) -> napi::Result<Span> {
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
}
