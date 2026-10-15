//! Converts projection maps, converter identities, and UTF-16 regions.

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use stillwind_ir::{
    AttributeQuote, ConverterIdentity, ImportRealm, ProjectionRegion, ScriptSyntax,
    SourceProjection,
};
use stillwind_source::{
    CoordinateIndex, FileId, ProjectionMap, SourceFile, SourceRegistry, Span, Utf16Offset,
    VolarMapping,
};

use super::NativeScriptSyntax;

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

impl From<NativeSourceProjection> for HostSourceProjection {
    fn from(value: NativeSourceProjection) -> Self {
        Self {
            code: value.code,
            syntax: value.syntax.into(),
            map: match value.map {
                NativeProjectionMap::V3 { json, source } => ProjectionMap::V3 {
                    json: (json).into(),
                    source: (source).into(),
                },
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
                name: (value.converter.name).into(),
                version: (value.converter.version).into(),
                options_fingerprint: (value.converter.options_fingerprint).into(),
            },
            import_realms: value.import_realms,
            editable_regions: value.editable_regions,
        }
    }
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
    pub(super) fn input(
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
pub(super) fn projection_span(
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
