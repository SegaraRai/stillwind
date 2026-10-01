//! Host transport for editable documents and prepared parser buffers.
//!
//! [`PreparedSource::input`] converts direct script documents or projected component
//! documents into analyzer input. Projection realms and editable regions arrive as
//! UTF-16 offsets in the original document and become validated byte spans through
//! [`CoordinateIndex`]. Surrogate interiors are rejected rather than rounded.
//! Source maps and converter records are supplied by the host.

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
#[napi(string_enum = "lowercase")]
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
#[napi(string_enum = "lowercase")]
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

/// Host input with an editable document and explicit parser preparation.
#[napi(
    discriminant = "kind",
    discriminant_case = "lowercase",
    object_to_js = false
)]
pub enum NativePreparedSource {
    /// Parses the editable script or JSX document directly.
    Direct {
        /// Editable source document.
        document: NativeSourceDocument,
        /// Parser settings for the document's code.
        syntax: NativeScriptSyntax,
        /// Host-chosen identity of this module.
        module_id: String,
        /// Facts the host produced for this module, empty when absent.
        type_facts: Option<Vec<NativeTypeFact>>,
        /// Explicit output adapter constraint; macro imports select it when absent.
        framework: Option<NativeFramework>,
        /// Host-allocated stylesheet modules to insert at the macro import.
        stylesheet_imports: Option<Vec<String>>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
    /// Parses code produced from an editable component document.
    Projected {
        /// Editable source document.
        document: NativeSourceDocument,
        /// Generated code and its correspondence to the document.
        projection: NativeSourceProjection,
        /// Host-chosen identity of this module.
        module_id: String,
        /// Facts the host produced for this module, empty when absent.
        type_facts: Option<Vec<NativeTypeFact>>,
        /// Explicit output adapter constraint; macro imports select it when absent.
        framework: Option<NativeFramework>,
        /// Host-allocated stylesheet modules to insert at the macro import.
        stylesheet_imports: Option<Vec<String>>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
}

impl TryFrom<NativePreparedSource> for PreparedSource {
    type Error = Error;

    fn try_from(value: NativePreparedSource) -> napi::Result<Self> {
        match value {
            NativePreparedSource::Direct {
                document,
                syntax,
                module_id,
                type_facts,
                framework,
                stylesheet_imports,
                module_css_import,
            } => Ok(Self::Direct {
                document: document.try_into()?,
                syntax: syntax.into(),
                module_id: module_id.into(),
                type_facts: type_facts
                    .unwrap_or_default()
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<napi::Result<_>>()?,
                framework: framework.map(Into::into),
                stylesheet_imports: stylesheet_imports.unwrap_or_default(),
                module_css_import,
            }),
            NativePreparedSource::Projected {
                document,
                projection,
                module_id,
                type_facts,
                framework,
                stylesheet_imports,
                module_css_import,
            } => Ok(Self::Projected {
                document: document.try_into()?,
                projection: projection.into(),
                module_id: module_id.into(),
                type_facts: type_facts
                    .unwrap_or_default()
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<napi::Result<_>>()?,
                framework: framework.map(Into::into),
                stylesheet_imports: stylesheet_imports.unwrap_or_default(),
                module_css_import,
            }),
        }
    }
}

/// Original editable document, including its optional upstream source map.
#[napi(object, object_to_js = false)]
pub struct NativeSourceDocument {
    /// Absolute URI of the editable document.
    pub uri: String,
    /// Editable text.
    pub code: String,
    /// Original file format, independent of the parser dialect.
    pub format: NativeSourceFormat,
    /// Source-map JSON from this document to its upstream source.
    pub upstream_map: Option<String>,
    /// Exact source name for generated stylesheet maps.
    pub source_map_source: Option<String>,
}

impl TryFrom<NativeSourceDocument> for SourceDocument {
    type Error = Error;

    fn try_from(value: NativeSourceDocument) -> napi::Result<Self> {
        Ok(Self {
            uri: SourceUri::parse(value.uri)
                .map_err(|error| Error::from_reason(error.to_string()))?,
            code: value.code,
            format: value.format.into(),
            upstream_map: value.upstream_map,
            source_map_source: value.source_map_source,
        })
    }
}

/// Parser dialect and module grammar for a script buffer.
#[napi(object, object_to_js = false)]
pub struct NativeScriptSyntax {
    /// JavaScript or TypeScript dialect.
    pub language: NativeScriptLanguage,
    /// Module grammar, independent of the dialect.
    pub module_kind: NativeScriptModuleKind,
}

impl From<NativeScriptSyntax> for ScriptSyntax {
    fn from(value: NativeScriptSyntax) -> Self {
        Self {
            language: value.language.into(),
            module_kind: value.module_kind.into(),
        }
    }
}

/// Generated parser buffer and required correspondence to the editable document.
#[napi(object, object_to_js = false)]
pub struct NativeSourceProjection {
    /// Generated JavaScript or TypeScript code.
    pub code: String,
    /// Parser settings for the generated code.
    pub syntax: NativeScriptSyntax,
    /// Mapping from generated code to the editable document.
    pub map: NativeProjectionMap,
    /// Converter identity used for the source revision.
    pub converter: NativeConverterIdentity,
    /// Import scopes in the editable document, in UTF-16 coordinates.
    pub import_realms: Vec<HostRealm>,
    /// Editable regions in the document, in UTF-16 coordinates.
    pub editable_regions: Vec<HostRegion>,
}

impl From<NativeSourceProjection> for SourceProjection {
    fn from(value: NativeSourceProjection) -> Self {
        Self {
            code: value.code,
            syntax: value.syntax.into(),
            map: match value.map {
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
            },
            converter: ConverterIdentity {
                name: value.converter.name,
                version: value.converter.version,
                options_fingerprint: value.converter.options_fingerprint,
            },
            import_realms: value.import_realms,
            editable_regions: value.editable_regions,
        }
    }
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
    pub options_fingerprint: String,
}

/// Prepared host input whose parser buffer is direct or projected.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PreparedSource {
    /// Parses an editable script or JSX document.
    Direct {
        /// Original editable document.
        document: SourceDocument,
        /// Parser settings for the document's code.
        syntax: ScriptSyntax,
        /// Host-chosen identity of this module.
        module_id: ModuleId,
        /// Facts the host produced for this module, empty when absent.
        #[serde(default)]
        type_facts: Vec<TypeFact>,
        /// Explicit output adapter constraint; macro imports select it when absent.
        framework: Option<AdapterKind>,
        /// Host-allocated stylesheet modules.
        #[serde(default)]
        stylesheet_imports: Vec<String>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
    /// Parses generated code while reporting and editing the component document.
    Projected {
        /// Original editable document.
        document: SourceDocument,
        /// Generated parser buffer and its correspondence to the document.
        projection: SourceProjection,
        /// Host-chosen identity of this module.
        module_id: ModuleId,
        /// Facts the host produced for this module, empty when absent.
        #[serde(default)]
        type_facts: Vec<TypeFact>,
        /// Explicit output adapter constraint; macro imports select it when absent.
        framework: Option<AdapterKind>,
        /// Host-allocated stylesheet modules.
        #[serde(default)]
        stylesheet_imports: Vec<String>,
        /// Host-allocated module that exports this module's CSS value.
        module_css_import: Option<String>,
    },
}

impl PreparedSource {
    /// Returns the host identity used for graph edges and duplicate checks.
    pub const fn module_id(&self) -> &ModuleId {
        match self {
            Self::Direct { module_id, .. } | Self::Projected { module_id, .. } => module_id,
        }
    }

    /// Returns the host facts used to finalize this module's scan.
    pub fn type_facts(&self) -> &[TypeFact] {
        match self {
            Self::Direct { type_facts, .. } | Self::Projected { type_facts, .. } => type_facts,
        }
    }

    /// Registers the parser and editable buffers and returns the analyzer input.
    ///
    /// Projection regions and realms must use UTF-16 offsets in the document.
    /// Published spans refer to that document. A projected parser buffer has its own
    /// file ID, and its analysis buffer receives a reserved ID in `sources`.
    /// Registration can remain in `sources` if a later coordinate conversion fails.
    ///
    /// # Errors
    ///
    /// Returns an error for a component document without a projection, a projected
    /// script or JSX document, invalid UTF-16 coordinates, reversed ranges, or
    /// exhausted file IDs.
    pub fn input(&self, sources: &mut SourceRegistry) -> napi::Result<SourceInput> {
        let (
            document,
            syntax,
            projection,
            module_id,
            framework,
            stylesheet_imports,
            module_css_import,
        ) = match self {
            Self::Direct {
                document,
                syntax,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
                ..
            } => {
                if document.format.projection_adapter().is_some() {
                    return Err(Error::from_reason(
                        "direct input requires a script or JSX document format",
                    ));
                }
                (
                    document,
                    syntax,
                    None,
                    module_id,
                    framework,
                    stylesheet_imports,
                    module_css_import,
                )
            }
            Self::Projected {
                document,
                projection,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
                ..
            } => {
                if document.format.projection_adapter().is_none() {
                    return Err(Error::from_reason(
                        "projected input requires a component document format",
                    ));
                }
                (
                    document,
                    &projection.syntax,
                    Some(projection),
                    module_id,
                    framework,
                    stylesheet_imports,
                    module_css_import,
                )
            }
        };
        let code = sources
            .register(
                if projection.is_some() {
                    None
                } else {
                    Some(document.uri.clone())
                },
                if projection.is_some() {
                    None
                } else {
                    document.source_map_source.as_deref().map(Into::into)
                },
                projection.map_or(document.code.as_str(), |projection| {
                    projection.code.as_str()
                }),
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let projection = projection
            .map(|projection| projection.input(document, sources))
            .transpose()?;
        Ok(SourceInput {
            module_id: module_id.clone(),
            stylesheet_imports: stylesheet_imports.clone(),
            module_css_import: module_css_import.clone(),
            code,
            language: syntax.language,
            module_kind: syntax.module_kind,
            source_format: document.format,
            framework: *framework,
            upstream_map: document.upstream_map.clone(),
            projection,
        })
    }
}

/// Original editable source and its optional upstream provenance.
#[derive(Deserialize)]
pub struct SourceDocument {
    /// Absolute URI of the editable document.
    pub uri: SourceUri,
    /// Editable text.
    pub code: String,
    /// Original file format, independent of the parser dialect.
    pub format: SourceFormat,
    /// Source-map JSON from this document to its upstream source.
    pub upstream_map: Option<String>,
    /// Exact source name for generated stylesheet maps.
    pub source_map_source: Option<String>,
}

/// Parser dialect and module grammar for a script buffer.
#[derive(Deserialize)]
pub struct ScriptSyntax {
    /// JavaScript or TypeScript dialect.
    pub language: ScriptLanguage,
    /// Module grammar; serde preserves `common-js`.
    pub module_kind: ScriptModuleKind,
}

/// Generated parser code with required mapping and converter metadata.
#[derive(Deserialize)]
pub struct SourceProjection {
    /// Generated JavaScript or TypeScript code.
    pub code: String,
    /// Parser settings for the generated code.
    pub syntax: ScriptSyntax,
    /// Mapping from generated code to the editable document.
    pub map: ProjectionMap,
    /// Converter identity used for the source revision.
    pub converter: ConverterIdentity,
    /// Import scopes in the document, in UTF-16 coordinates.
    pub import_realms: Vec<HostRealm>,
    /// Editable regions in the document, in UTF-16 coordinates.
    pub editable_regions: Vec<HostRegion>,
}

impl SourceProjection {
    /// Registers `document` as the original buffer and reserves an analysis ID.
    /// Returns a projection whose realm and region spans use original byte offsets.
    ///
    /// # Errors
    ///
    /// Returns an error for exhausted file IDs, invalid UTF-16 coordinates, or
    /// reversed ranges. Successful registrations remain in `sources` on failure.
    fn input(
        &self,
        document: &SourceDocument,
        sources: &mut SourceRegistry,
    ) -> napi::Result<FrameworkProjection> {
        let original = sources
            .register(
                Some(document.uri.clone()),
                document.source_map_source.as_deref().map(Into::into),
                document.code.as_str(),
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let original_id = original.id();
        let analysis_id = sources
            .reserve_generated_id()
            .map_err(|error| Error::from_reason(error.to_string()))?;
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
            .editable_regions
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
        Ok(FrameworkProjection {
            original,
            map: self.map.clone(),
            converter: self.converter.clone(),
            analysis_id,
            realms,
            regions,
        })
    }
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
    pub import_realm: u32,
    /// Delimiter of the enclosing attribute, when there is one.
    pub attribute_quote: Option<NativeAttributeQuote>,
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
