//! Converts editable documents, parser dialects, and adapter constraints.

use napi::Error;
use napi_derive::napi;
use serde::Deserialize;

use stillwind_ir::{AdapterKind, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceFormat};
use stillwind_source::SourceUri;

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
    /// Astro.
    Astro,
    /// Preact.
    Preact,
    /// React.
    React,
    /// Solid.
    Solid,
    /// Svelte.
    Svelte,
    /// Direct DOM consumers.
    Vanilla,
    /// Vue.
    Vue,
}
impl From<NativeFramework> for AdapterKind {
    fn from(value: NativeFramework) -> Self {
        match value {
            NativeFramework::Astro => Self::Astro,
            NativeFramework::Preact => Self::Preact,
            NativeFramework::React => Self::React,
            NativeFramework::Solid => Self::Solid,
            NativeFramework::Svelte => Self::Svelte,
            NativeFramework::Vanilla => Self::Vanilla,
            NativeFramework::Vue => Self::Vue,
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
