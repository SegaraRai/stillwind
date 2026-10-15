//! Editable source documents and the grammar of directly parsed scripts.

use std::sync::Arc;

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::SourceFile;

use super::SourceInputError;
use crate::AdapterKind;

/// The editable document and optional correspondence to an upstream original.
#[derive(Clone, Debug)]
pub struct SourceDocument {
    file: Arc<SourceFile>,
    format: SourceFormat,
    upstream_map: Option<LeanStr>,
}

impl SourceDocument {
    /// Takes an immutable file, its original format, and an optional upstream map.
    /// The upstream map is retained verbatim for consumers and does not change the edit target.
    pub fn new(file: Arc<SourceFile>, format: SourceFormat, map: Option<LeanStr>) -> Self {
        Self {
            file,
            format,
            upstream_map: map,
        }
    }

    /// Returns the buffer that receives edits and owns published source spans.
    pub fn file(&self) -> &Arc<SourceFile> {
        &self.file
    }

    /// Returns the format of the editable original document.
    pub const fn format(&self) -> SourceFormat {
        self.format
    }

    /// Returns the optional edited-source to upstream-original map without parsing it.
    pub fn upstream_map(&self) -> Option<&str> {
        self.upstream_map.as_deref()
    }
}

/// The language and module grammar of a JavaScript or TypeScript parser buffer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSyntax {
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
}

impl ScriptSyntax {
    /// Pairs the host's parser dialect and module grammar without filename inference.
    pub const fn new(language: ScriptLanguage, module_kind: ScriptModuleKind) -> Self {
        Self {
            language,
            module_kind,
        }
    }

    /// Returns the parser dialect supplied by the host.
    pub const fn language(self) -> ScriptLanguage {
        self.language
    }

    /// Returns the module grammar supplied by the host.
    pub const fn module_kind(self) -> ScriptModuleKind {
        self.module_kind
    }
}

/// A script document that can be parsed directly with the declared grammar.
#[derive(Clone, Debug)]
pub struct DirectSource {
    document: SourceDocument,
    syntax: ScriptSyntax,
}

impl DirectSource {
    /// Creates a direct script whose document format agrees with its parser dialect.
    pub(super) fn script(file: Arc<SourceFile>, syntax: ScriptSyntax) -> Self {
        Self {
            document: SourceDocument::new(file, SourceFormat::script(syntax.language()), None),
            syntax,
        }
    }

    /// Takes a document and parser grammar and guarantees a script or JSX format.
    ///
    /// # Errors
    ///
    /// Returns [`SourceInputError::InvalidDirectFormat`] for a component document.
    pub fn try_new(
        document: SourceDocument,
        syntax: ScriptSyntax,
    ) -> Result<Self, SourceInputError> {
        validate_direct_format(document.format())?;
        Ok(Self { document, syntax })
    }

    /// Returns the document whose bytes are parsed and edited.
    pub fn document(&self) -> &SourceDocument {
        &self.document
    }

    /// Returns the declared parser grammar.
    pub const fn syntax(&self) -> ScriptSyntax {
        self.syntax
    }
}

/// Checks whether `format` can be parsed directly and returns an error for components.
fn validate_direct_format(format: SourceFormat) -> Result<(), SourceInputError> {
    if format.projection_adapter().is_some() {
        Err(SourceInputError::InvalidDirectFormat { format })
    } else {
        Ok(())
    }
}

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
#[serde(rename_all = "lowercase")]
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

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    fn file(id: u32, text: &str) -> Arc<SourceFile> {
        Arc::new(SourceFile::new(FileId(id), None, None, text).unwrap())
    }

    fn syntax() -> ScriptSyntax {
        ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Module)
    }

    #[test]
    fn direct_sources_accept_script_formats_and_preserve_upstream_map_verbatim() {
        for format in [SourceFormat::Script, SourceFormat::Jsx] {
            let source = file(1, "const x = 1;");
            let direct = DirectSource::try_new(
                SourceDocument::new(source.clone(), format, Some("unparsed upstream map".into())),
                syntax(),
            )
            .unwrap();
            assert_eq!(direct.syntax(), syntax());
            assert_eq!(direct.document().format(), format);
            assert_eq!(
                direct.document().upstream_map(),
                Some("unparsed upstream map")
            );
            assert!(Arc::ptr_eq(direct.document().file(), &source));
        }
        for format in [SourceFormat::Svelte, SourceFormat::Astro, SourceFormat::Vue] {
            assert!(matches!(
                DirectSource::try_new(SourceDocument::new(file(1, ""), format, None), syntax()),
                Err(SourceInputError::InvalidDirectFormat { format: actual }) if actual == format
            ));
        }
    }
}
