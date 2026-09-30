//! Host input contract. Original framework sources are opaque text to the core.
//!
//! A host hands the analyzer one JavaScript/TypeScript buffer, plus optional records
//! describing how that buffer was derived from an original framework file. The core
//! never parses framework markup, never reads a file, and never resolves an import: it
//! works on the buffers and the correspondence the host declares here.
//!
//! Two coordinates must not be confused. [`SourceInput::code`] is what the parser sees
//! and what spans inside the parsed AST refer to; [`FrameworkProjection::original`] is
//! what the user edits and what every published span must refer to. A host without a
//! projection edits [`SourceInput::code`] directly.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use stillwind_source::{FileId, Fingerprint, ModuleId, ProjectionMap, SourceFile, Span};

use crate::AdapterKind;

/// Format of the editable original source, independent of its analysis buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    /// JavaScript or TypeScript without a JSX file extension.
    Script,
    /// A JSX or TSX file, including files without JSX expressions.
    Jsx,
    /// An Astro component.
    Astro,
    /// A Svelte component.
    Svelte,
    /// A Vue single-file component.
    Vue,
}

impl SourceFormat {
    /// Returns the original format for a plain script parser dialect.
    pub const fn script(language: ScriptLanguage) -> Self {
        match language {
            ScriptLanguage::Jsx | ScriptLanguage::Tsx => Self::Jsx,
            ScriptLanguage::Js | ScriptLanguage::Ts | ScriptLanguage::Dts => Self::Script,
        }
    }

    /// Returns the adapter whose normalization policy handles this component format.
    /// Plain scripts and JSX files have no component projection policy.
    pub const fn projection_adapter(self) -> Option<AdapterKind> {
        match self {
            Self::Script | Self::Jsx => None,
            Self::Astro => Some(AdapterKind::Astro),
            Self::Svelte => Some(AdapterKind::Svelte),
            Self::Vue => Some(AdapterKind::Vue),
        }
    }

    /// Returns the adapters accepted by the import lint, or an empty slice when
    /// any adapter is accepted for a reusable script module.
    pub const fn adapters(self) -> &'static [AdapterKind] {
        match self {
            Self::Script => &[],
            Self::Jsx => &[
                AdapterKind::React,
                AdapterKind::Preact,
                AdapterKind::Solid,
                AdapterKind::Vue,
            ],
            Self::Astro => &[AdapterKind::Astro],
            Self::Svelte => &[AdapterKind::Svelte],
            Self::Vue => &[AdapterKind::Vue],
        }
    }

    /// Returns the format label used in import diagnostics.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Script => "JavaScript/TypeScript",
            Self::Jsx => "JSX/TSX",
            Self::Astro => "Astro",
            Self::Svelte => "Svelte",
            Self::Vue => "Vue SFC",
        }
    }
}

/// JavaScript or TypeScript dialect of a parsed source buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLanguage {
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

/// Module grammar used when parsing a JavaScript or TypeScript buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptModuleKind {
    /// Parse as an ECMAScript module.
    Module,
    /// Parse with `CommonJS` top-level semantics.
    CommonJs,
    /// Parse as a non-module script.
    Script,
    /// Select module or script grammar from the buffer's syntax.
    Unambiguous,
}

/// Identifies the converter that produced a projection, for cache identity.
///
/// `options_fingerprint` must change whenever converter settings change the produced
/// mapping; otherwise a cached artifact can outlive the settings that justified it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConverterIdentity {
    /// Converter package or tool name.
    pub name: String,
    /// Converter version string.
    pub version: String,
    /// Fingerprint of the converter settings that produced the mapping.
    pub options_fingerprint: String,
}

/// An import scope of a converted framework file.
///
/// A realm is the set of imports visible at one place in the original file: the script
/// block of a Vue SFC, or one expression of an Astro template. Bindings resolve to the
/// innermost realm that contains them, so `parent` forms a tree rooted at the
/// file-level script scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportRealm {
    /// Host-chosen identifier, referenced by [`ProjectionRegion::import_realm`].
    pub id: u32,
    /// Span of the realm in the original source.
    pub source: Span,
    /// Enclosing script scope whose imports are visible in this realm.
    pub parent: Option<u32>,
}

/// A region of the original file that was converted into JavaScript.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectionRegion {
    /// Span of the converted region in the original file.
    pub source: Span,
    /// Realm whose imports are visible inside this region.
    pub import_realm: u32,
    /// Quote style of the enclosing attribute, when the region is an attribute value
    /// and the converter must reproduce the original delimiters.
    pub attribute_quote: Option<AttributeQuote>,
}

/// Quote character that delimited a converted attribute value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributeQuote {
    /// The region was written inside single quotes.
    Single,
    /// The region was written inside double quotes.
    Double,
}

/// Correspondence between the parsed buffer and the original framework file.
#[derive(Clone, Debug)]
pub struct FrameworkProjection {
    /// The file the user edits and the file every published span refers to.
    pub original: Arc<SourceFile>,
    /// Generated-to-original mapping for the converted regions.
    pub map: ProjectionMap,
    /// Converter identity, part of the input revision.
    pub converter: ConverterIdentity,
    /// [`FileId`] used by spans produced while analyzing the generated buffer. Spans
    /// carrying this id must be translated before they leave the frontend.
    pub analysis_id: FileId,
    /// Import scopes of the original file, referenced by regions.
    pub realms: Vec<ImportRealm>,
    /// Converted regions of the original file.
    pub regions: Vec<ProjectionRegion>,
}

/// One module as presented to the analyzer.
#[derive(Clone, Debug)]
pub struct SourceInput {
    /// Compiler identity of the module represented by the input.
    pub module_id: ModuleId,
    /// JavaScript/TypeScript presented to the parser, never raw SFC markup.
    pub code: Arc<SourceFile>,
    /// Dialect the parser must use for [`Self::code`].
    pub language: ScriptLanguage,
    /// Module grammar supplied by the host for the parser.
    pub module_kind: ScriptModuleKind,
    /// Format of the editable original, used for projection policy and import lints.
    pub source_format: SourceFormat,
    /// Explicit output adapter constraint. Macro imports select it when absent.
    pub framework: Option<AdapterKind>,
    /// Present when [`Self::code`] was derived from another file.
    pub projection: Option<FrameworkProjection>,
    /// Edited-source to upstream-original map. Does not change the edit target.
    pub upstream_map: Option<String>,
    /// Host-allocated stylesheet modules inserted at a verified macro import.
    pub stylesheet_imports: Vec<String>,
    /// Host-allocated JavaScript module exporting this module's CSS value.
    pub module_css_import: Option<String>,
}

impl SourceInput {
    /// Returns plain script input without a projection or explicit adapter constraint.
    /// The original format defaults from `language`; module identities are opaque.
    pub fn normal(
        code: Arc<SourceFile>,
        module_id: ModuleId,
        language: ScriptLanguage,
        module_kind: ScriptModuleKind,
    ) -> Self {
        Self {
            module_id,
            language,
            module_kind,
            source_format: SourceFormat::script(language),
            code,
            framework: None,
            projection: None,
            upstream_map: None,
            stylesheet_imports: Vec::new(),
            module_css_import: None,
        }
    }

    /// The buffer that edits apply to: the projection original when present, otherwise
    /// the parsed buffer itself.
    pub fn edit_source(&self) -> &Arc<SourceFile> {
        self.projection
            .as_ref()
            .map_or(&self.code, |projection| &projection.original)
    }

    /// Includes source, parser, original format, framework, map, converter settings and realms. Debug
    /// representations contain values only (no addresses or host state).
    ///
    /// Hosts use this as a cache key component; two inputs with the same revision are
    /// interchangeable for analysis.
    pub fn revision(&self) -> Fingerprint {
        Fingerprint::of(format!("{self:?}"))
    }
}
