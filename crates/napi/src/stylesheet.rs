use std::collections::BTreeMap;

use crate::output::NativeModuleResource;
use napi_derive::napi;
use stillwind_compiler::StylesheetMapping;

#[napi(
    discriminant = "kind",
    discriminant_case = "camelCase",
    object_from_js = false
)]
pub enum NativeCssLoadStep {
    NeedFile {
        requests: Vec<NativeCssRequest>,
    },
    Complete {
        revision: String,
        resources: Vec<NativeCssResource>,
        sources: Vec<String>,
    },
}

#[napi(object, object_from_js = false)]
pub struct NativeCssRequest {
    pub id: u32,
    pub importer: String,
    pub specifier: String,
}

#[napi(object, object_from_js = false)]
pub struct NativeCssResource {
    pub placeholder: String,
    pub owner: String,
    pub specifier: String,
    pub start: u32,
    pub end: u32,
}

#[napi(object, object_from_js = false)]
pub struct NativeReload {
    pub css_changed: bool,
    pub transformed_modules: Vec<String>,
}

#[napi(discriminant = "kind", discriminant_case = "camelCase")]
pub enum NativeCoverage {
    Observed,
    Complete { target: String, revision: String },
}

#[napi(object, object_to_js = false)]
pub struct NativeStylesheetSelection {
    pub modules: Vec<String>,
    pub coverage: NativeCoverage,
    pub placement: NativeStylesheetPlacement,
}

#[napi(string_enum = "lowercase")]
pub enum NativeStylesheetPlacement {
    Document,
    Carrier,
}

#[napi(object, object_from_js = false)]
pub struct NativeStylesheetPlan {
    pub css: String,
    pub source_map: NativeStylesheetMap,
    pub resources: Vec<NativeModuleResource>,
    pub coverage: NativeCoverage,
    pub placement: NativeStylesheetPlacement,
    pub design_system: String,
    pub producers: Vec<NativeStylesheetProducer>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
pub struct NativeStylesheetProducer {
    pub module: String,
    pub source_revision: String,
    pub semantic_revision: Option<String>,
    pub compiler_version: String,
    pub compiler_options: String,
    pub analysis_revision: String,
}

#[napi(object, object_from_js = false)]
pub struct NativeStylesheetOutput {
    pub css: String,
    pub source_map: NativeStylesheetMap,
}

#[napi(object, object_from_js = false)]
pub struct NativeStylesheetMap {
    pub sources: Vec<NativeMapSource>,
    pub entries: Vec<NativeMapEntry>,
}

#[napi(object, object_from_js = false)]
pub struct NativeMapSource {
    pub path: String,
    pub content: String,
}

#[napi(object, object_from_js = false)]
pub struct NativeMapEntry {
    pub source: u32,
    pub generated_start: u32,
    pub generated_end: u32,
    pub original_start: u32,
    pub original_end: u32,
    pub exact: bool,
}

pub(crate) fn stylesheet_locations(
    mappings: &[StylesheetMapping],
) -> napi::Result<NativeStylesheetMap> {
    let mut sources = Vec::new();
    let mut indices = BTreeMap::new();
    let mut entries = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let offset = |value| {
            u32::try_from(value)
                .map_err(|_| napi::Error::from_reason("Stylesheet map offset exceeds 32 bits"))
        };
        let next = u32::try_from(sources.len())
            .map_err(|_| napi::Error::from_reason("Too many stylesheet sources"))?;
        let index = *indices
            .entry(mapping.source.path().to_owned())
            .or_insert_with(|| {
                sources.push(NativeMapSource {
                    path: mapping.source.path().into(),
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
