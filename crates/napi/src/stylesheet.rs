//! Transport types for the CSS load protocol and for stylesheet output.
//!
//! These records mirror the Rust domain types the JavaScript host already works with:
//! the loader step, a host selection, and a rendered plan. Offsets crossing back to
//! JavaScript are published twice where a consumer may need either convention, and
//! spans always use byte offsets in a named source.

use std::collections::BTreeMap;

use napi_derive::napi;

use stillwind_compiler::StylesheetMapping;

use crate::output::NativeModuleResource;

/// One step of the design-system load state machine.
///
/// [`Self::NeedFile`] carries every outstanding request; [`Self::Complete`] is terminal
/// and reports what was read. Parse, link, and protocol failures are reported as a
/// JavaScript error instead, so there is no error arm here.
#[napi(
    discriminant = "kind",
    discriminant_case = "camelCase",
    object_from_js = false
)]
pub enum NativeCssLoadStep {
    /// The host must resolve and read these sources next.
    NeedFile {
        /// Outstanding requests, ordered by id.
        requests: Vec<NativeCssRequest>,
    },
    /// The load finished and a snapshot is available.
    Complete {
        /// Revision of the completed snapshot.
        revision: String,
        /// Resource references the CSS declares.
        resources: Vec<NativeCssResource>,
        /// Names of every source the loader received, for watch registration.
        sources: Vec<String>,
    },
}

/// One unresolved CSS import.
#[napi(object, object_from_js = false)]
pub struct NativeCssRequest {
    /// Id the host must answer with.
    pub id: u32,
    /// Opaque name of the source that contains the import.
    pub importer: String,
    /// Decoded import specifier, without CSS quoting or escapes.
    pub specifier: String,
}

/// A resource reference found in authored CSS.
#[napi(object, object_from_js = false)]
pub struct NativeCssResource {
    /// Placeholder left in the CSS that the host must substitute.
    pub placeholder: String,
    /// Opaque name of the source that owns the reference.
    #[napi(js_name = "source_name")]
    pub source_name: String,
    /// Authored `url()` specifier.
    pub specifier: String,
    /// Byte start of the complete authored resource expression in the owning source.
    pub start: u32,
    /// Exclusive byte end of the complete authored resource expression.
    pub end: u32,
}

/// What changed after publishing a reloaded design system.
#[napi(object, object_from_js = false)]
pub struct NativeReload {
    /// Whether the assembled CSS differs from the previous revision.
    pub css_changed: bool,
    /// Modules whose code transforms changed.
    pub transformed_modules: Vec<String>,
}

/// Completeness claim attached to a host selection.
#[napi(discriminant = "kind", discriminant_case = "camelCase")]
pub enum NativeCoverage {
    /// Only what this session has seen; no claim about unvisited modules.
    Observed,
    /// The host guarantees this set for this target and graph revision.
    Complete {
        /// Host-defined identity of the output being planned.
        target: String,
        /// Host-defined revision of the module graph.
        revision: String,
    },
}

/// Producers, coverage claim, and placement for one output.
#[napi(object, object_to_js = false)]
pub struct NativeStylesheetSelection {
    /// Module paths to include.
    pub modules: Vec<String>,
    /// What the host knows about the completeness of `modules`.
    pub coverage: NativeCoverage,
    /// Where the rendered CSS will be placed.
    pub placement: NativeStylesheetPlacement,
}

/// Where a rendered stylesheet is meant to live.
#[napi(string_enum = "lowercase")]
pub enum NativeStylesheetPlacement {
    /// A complete document stylesheet, including theme and base rules.
    Document,
    /// A fragment carried by a module, containing only selected producers.
    Carrier,
}

/// A rendered stylesheet with provenance and watch information.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetPlan {
    /// Ordered occurrences. Repeated payloads retain their positions.
    pub parts: Vec<NativeStylesheetPart>,
    /// The rendered CSS text.
    pub css: String,
    /// Source locations for the rules that have one.
    pub source_map: NativeStylesheetMap,
    /// Assets and resources the selected producers reference.
    pub resources: Vec<NativeModuleResource>,
    /// Coverage claim carried over from the selection.
    pub coverage: NativeCoverage,
    /// Placement this plan was rendered for.
    pub placement: NativeStylesheetPlacement,
    /// Content fingerprint of the design system used.
    pub design_system: String,
    /// One entry per selected producer, for cache identity.
    pub producers: Vec<NativeStylesheetProducer>,
}

/// A CSS fragment at one position in an output plan.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetPart {
    /// Source locations relative to this wrapped fragment.
    pub source_map: NativeStylesheetMap,
    /// Stable template node ordinal.
    pub slot: u32,
    /// Zero-based occurrence index for this template slot and producer set.
    pub sequence: u32,
    /// Producer paths contributing to this occurrence; empty for shared support.
    pub producers: Vec<String>,
    /// Content fingerprint.
    pub key: String,
    /// Complete CSS including enclosing context.
    pub css: String,
}

/// Artifact identity of one selected producer.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct NativeStylesheetProducer {
    /// Module path.
    pub module: String,
    /// Content revision of the module source.
    pub source_revision: String,
    /// Revision of the semantic facts used, when the module had any.
    pub semantic_revision: Option<String>,
    /// Compiler build fingerprint.
    pub compiler_version: String,
    /// Fingerprint of the resolved compilation settings.
    pub compiler_options: String,
    /// Fingerprint of the finalized analysis the artifact was built from.
    pub analysis_revision: String,
}

/// Rendered CSS with its source map, without a selection.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetOutput {
    /// The rendered CSS text.
    pub css: String,
    /// Source locations for the rules that have one.
    pub source_map: NativeStylesheetMap,
}

/// Source map for generated CSS.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetMap {
    /// Distinct sources referenced by the entries.
    pub sources: Vec<NativeMapSource>,
    /// Mappings, in generated order.
    pub entries: Vec<NativeMapEntry>,
}

/// One source referenced by a stylesheet map.
#[napi(object, object_from_js = false)]
pub struct NativeMapSource {
    /// Host-supplied source-map entry name for this source revision.
    pub source_map_source: String,
    /// Full text, so a consumer can build a standard source map without reading the
    /// file again.
    pub content: String,
}

/// One generated-to-original mapping in byte offsets.
#[napi(object, object_from_js = false)]
pub struct NativeMapEntry {
    /// Index into [`NativeStylesheetMap::sources`].
    pub source: u32,
    /// Byte start in the generated CSS.
    pub generated_start: u32,
    /// Byte end in the generated CSS.
    pub generated_end: u32,
    /// Byte start in the original source.
    pub original_start: u32,
    /// Byte end in the original source.
    pub original_end: u32,
    /// Whether the mapping is exact rather than a whole-rule approximation.
    pub exact: bool,
}

/// Converts compiler mappings into the transport map.
///
/// Sources are deduplicated by path and assigned indices in first-use order, so the
/// generated map stays as small as the original mapping list allows. Offsets that do
/// not fit in `u32` are rejected instead of truncated.
pub(crate) fn stylesheet_locations(
    mappings: &[StylesheetMapping],
) -> napi::Result<NativeStylesheetMap> {
    let mut sources = Vec::new();
    let mut indices = BTreeMap::new();
    let mut entries = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let source_map_source = mapping.source.source_map_source().ok_or_else(|| {
            napi::Error::from_reason("Stylesheet source is missing source_map_source")
        })?;
        let offset = |value| {
            u32::try_from(value)
                .map_err(|_| napi::Error::from_reason("Stylesheet map offset exceeds 32 bits"))
        };
        let next = u32::try_from(sources.len())
            .map_err(|_| napi::Error::from_reason("Too many stylesheet sources"))?;
        let index = *indices.entry(mapping.source.id()).or_insert_with(|| {
            sources.push(NativeMapSource {
                source_map_source: source_map_source.into(),
                content: mapping.source.text().into(),
            });
            next
        });
        entries.push(NativeMapEntry {
            source: index,
            generated_start: offset(mapping.generated.start)?,
            generated_end: offset(mapping.generated.end)?,
            original_start: offset(mapping.original.start)?,
            original_end: offset(mapping.original.end)?,
            exact: mapping.exact,
        });
    }
    Ok(NativeStylesheetMap { sources, entries })
}
