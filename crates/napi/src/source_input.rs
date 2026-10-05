//! Host transport for editable documents and prepared parser buffers.
//!
//! [`HostPreparedSource::input`] converts direct script documents or projected
//! component documents into analyzer input. Projection realms and editable regions
//! arrive as UTF-16 offsets in the original document and become byte spans through
//! [`CoordinateIndex`]. Surrogate interiors are rejected rather than rounded. Core
//! constructors validate source formats and projection metadata. The host supplies
//! source maps and converter records.

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;
use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{
    AdapterKind, AttributeQuote, ConverterIdentity, DirectSource, ImportRealm, PreparedSource,
    ProjectedSource, ProjectionRegion, ScriptLanguage, ScriptModuleKind, ScriptSyntax,
    SourceDocument, SourceFormat, SourceInput, SourceProjection,
};
use stillwind_semantic::TypeFact;
use stillwind_source::{
    CoordinateIndex, FileId, ModuleId, ProjectionMap, SourceFile, SourceRegistry, SourceUri, Span,
    Utf16Offset, VolarMapping,
};

use crate::semantic::NativeTypeFact;

/// Known source failures reported by JavaScript projection and checker adapters.
#[napi(
    discriminant = "kind",
    discriminant_case = "lowercase",
    object_to_js = false
)]
pub enum NativeSourceIssue {
    /// Original component syntax was rejected by its parser.
    Syntax { detail: String },
    /// A projection or checker buffer could not be prepared.
    Projection { detail: String },
    /// An original expression has no required checker target.
    Checker { detail: String },
    /// An assignment lost compatible declaration identities.
    Assignment { detail: String },
    /// A value does not carry a finite set of declaration identities.
    Identity { detail: String },
}

impl From<NativeSourceIssue> for CompileDiagnostic {
    fn from(value: NativeSourceIssue) -> Self {
        match value {
            NativeSourceIssue::Syntax { detail } => Self::ComponentSyntax { detail },
            NativeSourceIssue::Projection { detail } => Self::ProjectionPreparation { detail },
            NativeSourceIssue::Checker { detail } => Self::CheckerTargetUnavailable { detail },
            NativeSourceIssue::Assignment { detail } => Self::IdentityAssignment { detail },
            NativeSourceIssue::Identity { detail } => Self::IdentityUnavailable { detail },
        }
    }
}

/// Half-open UTF-16 offsets in the original document; surrogate interiors are invalid.
#[napi(object, object_to_js = false)]
pub struct NativeOriginalRange {
    pub start: u32,
    pub end: u32,
}

/// A known failure before native compilation, tied to the attempted original buffer.
#[napi(object, object_to_js = false)]
pub struct NativeSourceFailure {
    pub module_id: String,
    pub document: NativeSourceDocument,
    pub issue: NativeSourceIssue,
    /// Absent when the adapter cannot prove a precise original location.
    pub range: Option<NativeOriginalRange>,
    /// Absent for direct scripts or failures before a converter could be loaded.
    pub converter: Option<NativeConverterIdentity>,
}

impl NativeSourceFailure {
    /// Registers the original revision and validates any supplied UTF-16 range.
    /// Returns a typed failure with converter context, or rejects malformed host input.
    pub(crate) fn resolve(
        self,
        sources: &mut SourceRegistry,
    ) -> napi::Result<(ModuleId, Arc<SourceFile>, CompileError)> {
        let document = HostSourceDocument::try_from(self.document)?;
        let source = sources
            .register(
                Some(document.uri),
                document.source_map_source.as_deref().map(Into::into),
                document.code,
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let mut error = if let Some(range) = self.range {
            let span = projection_span(
                &CoordinateIndex::new(&source),
                source.id(),
                range.start,
                range.end,
            )?;
            source
                .slice(span)
                .map_err(|error| Error::from_reason(error.to_string()))?;
            CompileError::new(self.issue.into(), span)
        } else {
            CompileError::for_document(self.issue.into(), &source)
        };
        error
            .notes
            .push(format!("Source format: {:?}", document.format));
        if let Some(converter) = self.converter {
            error.notes.push(format!(
                "Converter: {}@{}",
                converter.name, converter.version
            ));
        }
        Ok((ModuleId::from(self.module_id), source, error))
    }
}

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

impl TryFrom<NativePreparedSource> for HostPreparedSource {
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

impl TryFrom<NativeSourceDocument> for HostSourceDocument {
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
        Self::new(value.language.into(), value.module_kind.into())
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

impl From<NativeSourceProjection> for HostSourceProjection {
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

/// Host transport whose parser buffer is direct or projected.
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum HostPreparedSource {
    /// Parses an editable script or JSX document.
    Direct {
        /// Original editable document.
        document: HostSourceDocument,
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
        document: HostSourceDocument,
        /// Generated parser buffer and its correspondence to the document.
        projection: HostSourceProjection,
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

impl HostPreparedSource {
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
    /// Projected inputs register the parser buffer first, the document second, and
    /// reserve the analysis ID third. Registrations remain in `sources` on failure.
    ///
    /// # Errors
    ///
    /// Returns an error for a component document without a projection, a projected
    /// script or JSX document, invalid UTF-16 coordinates, reversed ranges, exhausted
    /// file IDs, or invalid projection metadata rejected by core constructors.
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
            } => (
                document,
                syntax,
                None,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
            ),
            Self::Projected {
                document,
                projection,
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
                ..
            } => (
                document,
                &projection.syntax,
                Some(projection),
                module_id,
                framework,
                stylesheet_imports,
                module_css_import,
            ),
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
        let prepared = if let Some(projection) = projection {
            let original = sources
                .register(
                    Some(document.uri.clone()),
                    document.source_map_source.as_deref().map(Into::into),
                    document.code.as_str(),
                )
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let projection = projection.input(code, &original, sources)?;
            let document =
                SourceDocument::new(original, document.format, document.upstream_map.clone());
            PreparedSource::Projected(
                ProjectedSource::try_new(document, projection)
                    .map_err(|error| Error::from_reason(error.to_string()))?,
            )
        } else {
            let document =
                SourceDocument::new(code, document.format, document.upstream_map.clone());
            PreparedSource::Direct(
                DirectSource::try_new(document, *syntax)
                    .map_err(|error| Error::from_reason(error.to_string()))?,
            )
        };
        let mut input = SourceInput::new(module_id.clone(), prepared);
        input.framework = *framework;
        input.stylesheet_imports.clone_from(stylesheet_imports);
        input.module_css_import.clone_from(module_css_import);
        Ok(input)
    }
}

/// Host transport for an editable source and its optional upstream provenance.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSourceDocument {
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

/// Host transport for generated parser code, mapping, and UTF-16 metadata.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSourceProjection {
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

impl HostSourceProjection {
    /// Converts UTF-16 realms and regions against `original` and constructs a core
    /// projection. `code` must be the registered parser buffer, and `original` must be
    /// its editable document. Reserves an analysis ID in `sources`; the reservation
    /// remains on failure.
    ///
    /// # Errors
    ///
    /// Returns an error for exhausted file IDs, invalid coordinates, or invalid core
    /// metadata.
    fn input(
        &self,
        code: std::sync::Arc<SourceFile>,
        original: &SourceFile,
        sources: &mut SourceRegistry,
    ) -> napi::Result<SourceProjection> {
        let original_id = original.id();
        let analysis_id = sources
            .reserve_generated_id()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let index = CoordinateIndex::new(original);
        let span = |start, end| projection_span(&index, original_id, start, end);
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
        SourceProjection::try_new(
            code,
            self.syntax,
            self.map.clone(),
            self.converter.clone(),
            analysis_id,
            realms,
            regions,
        )
        .map_err(|error| Error::from_reason(error.to_string()))
    }
}

/// Source offsets in the host transport are UTF-16; native spans are bytes.
#[napi(object, object_to_js = false)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

/// Converts original-source UTF-16 offsets into a byte span for `original_id`.
/// The core constructors validate the resulting span's ownership and ordering.
///
/// # Errors
///
/// Returns an error when either offset is invalid or lies inside a surrogate pair.
fn projection_span(
    index: &CoordinateIndex,
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
    Ok(Span::new(original_id, start.0, end.0))
}
