//! Transport types for the CSS load protocol and for stylesheet output.
//!
//! These records mirror the Rust domain types the JavaScript host already works with:
//! the loader step, a host selection, and a rendered plan. Offsets crossing back to
//! JavaScript are published twice where a consumer may need either convention, and
//! spans always use byte offsets in a named source.

use std::collections::BTreeMap;

use napi::bindgen_prelude::{Env, JsObjectValue};
use napi::{Error, Status};
use napi_derive::napi;

use stillwind_compiler::{RenderedStylesheet, StylesheetMapping, StylesheetRenderError};
use stillwind_design_system::{CssDependencyKind, CssFileRequest};
use stillwind_project::{StylesheetCoverage, StylesheetPlan};

use crate::output::{BindingRejectedIdentifier, NativeModuleResource, NativeText};

/// Throws an assembly error with its fallback reports attached as `rejectedNames`.
/// Returns the pending-exception status so native and WASM bindings preserve the same JS object.
pub(crate) fn stylesheet_error(env: Env, error: StylesheetRenderError) -> Error {
    let create = || -> napi::Result<Error> {
        let mut value = env.create_error(Error::from_reason(error.to_string()))?;
        value.set_named_property(
            "rejectedNames",
            error
                .rejected_names
                .into_iter()
                .map(BindingRejectedIdentifier::from)
                .collect::<Vec<_>>(),
        )?;
        env.throw(value)?;
        Ok(Error::new(
            Status::PendingException,
            "Stylesheet rendering failed",
        ))
    };
    create().unwrap_or_else(|error| error)
}

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
        /// Names of the sources marked for stylesheet lint, in acceptance order.
        lint_sources: Vec<String>,
    },
}

/// One unresolved CSS import.
#[napi(object, object_from_js = false)]
pub struct NativeCssRequest {
    /// Id the host must answer with.
    pub id: u32,
    /// Opaque name of the source that contains the import.
    #[napi(ts_type = "string")]
    pub importer: NativeText,
    /// Decoded import or plugin specifier, without CSS quoting or escapes.
    #[napi(ts_type = "string")]
    pub specifier: NativeText,
    /// Declarations of an `@plugin` options block in authored order, and empty without
    /// a block. Absent for an `@import`, whose answer is the imported CSS file.
    pub plugin_options: Option<Vec<NativePluginOption>>,
}

impl From<CssFileRequest> for NativeCssRequest {
    fn from(request: CssFileRequest) -> Self {
        Self {
            id: request.id,
            importer: (request.importer.as_str().to_owned()).into(),
            specifier: (request.specifier).into(),
            plugin_options: match request.kind {
                CssDependencyKind::Import => None,
                CssDependencyKind::Plugin { options } => Some(
                    options
                        .into_iter()
                        .map(|option| NativePluginOption {
                            name: option.name.into(),
                            value: option.value.into(),
                        })
                        .collect(),
                ),
            },
        }
    }
}

/// One `name: value;` declaration of a `@plugin` options block.
#[napi(object, object_from_js = false)]
pub struct NativePluginOption {
    /// Option name with CSS escapes decoded.
    #[napi(ts_type = "string")]
    pub name: NativeText,
    /// Authored value text without the semicolon, trimmed. Each run of whitespace and
    /// comments between top-level tokens becomes one space.
    #[napi(ts_type = "string")]
    pub value: NativeText,
}

/// A resource reference found in authored CSS.
#[napi(object, object_from_js = false)]
pub struct NativeCssResource {
    /// Placeholder left in the CSS that the host must substitute.
    #[napi(ts_type = "string")]
    pub placeholder: NativeText,
    /// Opaque name of the source that owns the reference.
    #[napi(ts_type = "string")]
    pub source_name: NativeText,
    /// Authored `url()` specifier.
    #[napi(ts_type = "string")]
    pub specifier: NativeText,
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
    #[napi(ts_type = "Array<string>")]
    pub transformed_modules: Vec<NativeText>,
}

/// Completeness claim attached to a host selection.
#[napi(discriminant = "kind", discriminant_case = "lowercase")]
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
    #[napi(ts_type = "string")]
    pub css: NativeText,
    /// Development fallbacks observed during this plan's renders, in request order.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
    /// Source locations for the rules that have one.
    pub source_map: NativeStylesheetMap,
    /// Assets and resources the selected producers reference.
    pub resources: Vec<NativeModuleResource>,
    /// Coverage claim carried over from the selection.
    pub coverage: NativeCoverage,
    /// Placement this plan was rendered for.
    pub placement: NativeStylesheetPlacement,
    /// Content fingerprint of the design system used.
    #[napi(ts_type = "string")]
    pub design_system: NativeText,
    /// One entry per selected producer, for cache identity.
    pub producers: Vec<NativeStylesheetProducer>,
}

impl NativeStylesheetPlan {
    /// Converts a validated core plan, omitting map entries and sources when maps are disabled.
    /// Returns an error for offsets outside the host range or unresolved map sources.
    pub(crate) fn from_plan(
        plan: StylesheetPlan,
        placement: NativeStylesheetPlacement,
        include_source_maps: bool,
    ) -> napi::Result<Self> {
        let coverage = match plan.coverage {
            StylesheetCoverage::Observed => NativeCoverage::Observed,
            StylesheetCoverage::Complete { target, revision } => NativeCoverage::Complete {
                target: (target).into(),
                revision: (revision).into(),
            },
        };
        Ok(Self {
            parts: plan
                .parts
                .into_iter()
                .map(|part| {
                    Ok(NativeStylesheetPart {
                        source_map: stylesheet_locations(if include_source_maps {
                            &part.mappings
                        } else {
                            &[]
                        })?,
                        slot: u32::try_from(part.slot)
                            .map_err(|_| Error::from_reason("Stylesheet slot exceeds 32 bits"))?,
                        sequence: u32::try_from(part.sequence).map_err(|_| {
                            Error::from_reason("Stylesheet sequence exceeds 32 bits")
                        })?,
                        key: (part.key.0).into(),
                        css: (part.css).into(),
                        producers: part
                            .producers
                            .into_iter()
                            .map(|module| module.into_string().into())
                            .collect(),
                    })
                })
                .collect::<napi::Result<_>>()?,
            css: (plan.css).into(),
            rejected_names: plan.rejected_names.into_iter().map(Into::into).collect(),
            source_map: stylesheet_locations(if include_source_maps {
                &plan.mappings
            } else {
                &[]
            })?,
            resources: plan.resources.into_iter().map(Into::into).collect(),
            coverage,
            placement,
            design_system: (plan.design_system.0).into(),
            producers: plan
                .producers
                .into_iter()
                .map(|(module, key)| NativeStylesheetProducer {
                    module: (module.into_string()).into(),
                    source_revision: (key.source_revision().0.clone()).into(),
                    semantic_revision: (key
                        .semantic_revision()
                        .as_ref()
                        .map(|revision| revision.0.clone()))
                    .map(Into::into),
                    compiler_version: (key.compiler_version().to_owned()).into(),
                    compiler_options: (key.compiler_options().0.clone()).into(),
                    analysis_revision: (key.analysis_revision().0.clone()).into(),
                })
                .collect(),
        })
    }
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
    #[napi(ts_type = "Array<string>")]
    pub producers: Vec<NativeText>,
    /// Content fingerprint.
    #[napi(ts_type = "string")]
    pub key: NativeText,
    /// Complete CSS including enclosing context.
    #[napi(ts_type = "string")]
    pub css: NativeText,
}

/// Artifact identity of one selected producer.
#[napi(object, object_from_js = false, use_nullable = true)]
pub struct NativeStylesheetProducer {
    /// Module path.
    #[napi(ts_type = "string")]
    pub module: NativeText,
    /// Content revision of the module source.
    #[napi(ts_type = "string")]
    pub source_revision: NativeText,
    /// Revision of the semantic facts used, when the module had any.
    #[napi(ts_type = "string")]
    pub semantic_revision: Option<NativeText>,
    /// Compiler build fingerprint.
    #[napi(ts_type = "string")]
    pub compiler_version: NativeText,
    /// Fingerprint of the resolved compilation settings.
    #[napi(ts_type = "string")]
    pub compiler_options: NativeText,
    /// Fingerprint of the finalized analysis the artifact was built from.
    #[napi(ts_type = "string")]
    pub analysis_revision: NativeText,
}

/// Rendered CSS with its source map, without a selection.
#[napi(object, object_from_js = false)]
pub struct NativeStylesheetOutput {
    /// The rendered CSS text.
    #[napi(ts_type = "string")]
    pub css: NativeText,
    /// Source locations for the rules that have one.
    pub source_map: NativeStylesheetMap,
    /// Development fallbacks observed during this render, in request order.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
}

impl NativeStylesheetOutput {
    /// Converts rendered CSS and fallback reports, omitting mappings when the host does not need them.
    /// Returns an error for offsets outside the host range or unresolved map sources.
    pub(crate) fn from_render(
        output: RenderedStylesheet,
        include_source_maps: bool,
    ) -> napi::Result<Self> {
        Ok(Self {
            css: (output.stylesheet.css).into(),
            source_map: stylesheet_locations(if include_source_maps {
                &output.stylesheet.mappings
            } else {
                &[]
            })?,
            rejected_names: output.rejected_names.into_iter().map(Into::into).collect(),
        })
    }
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
    #[napi(ts_type = "string")]
    pub source_map_source: NativeText,
    /// Full text, so a consumer can build a standard source map without reading the
    /// file again.
    #[napi(ts_type = "string")]
    pub content: NativeText,
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
        // Internal buffers, such as plugin CSS, have no source-map identity, so their
        // generated text stays unmapped.
        let Some(source_map_source) = mapping.source.source_map_source() else {
            continue;
        };
        let offset = |value| {
            u32::try_from(value)
                .map_err(|_| napi::Error::from_reason("Stylesheet map offset exceeds 32 bits"))
        };
        let next = u32::try_from(sources.len())
            .map_err(|_| napi::Error::from_reason("Too many stylesheet sources"))?;
        let index = *indices.entry(mapping.source.id()).or_insert_with(|| {
            sources.push(NativeMapSource {
                source_map_source: source_map_source.into(),
                content: NativeText::source_text(mapping.source.clone()),
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
