//! Module inputs that combine validated source preparations with output settings.

use std::sync::Arc;

use stillwind_source::{FileId, Fingerprint, ModuleId, SourceError, SourceFile};

use super::{
    DirectSource, ProjectedSource, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceDocument,
    SourceFormat, SourceProjection,
};
use crate::AdapterKind;

/// A violation of the source format, identity, or import metadata contract.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceInputError {
    /// A direct input names a component format.
    #[error("direct source requires script or JSX format, received {format:?}")]
    InvalidDirectFormat {
        /// Rejected document format.
        format: SourceFormat,
    },
    /// A projected input names a plain script format.
    #[error("projected source requires a component format, received {format:?}")]
    InvalidProjectedFormat {
        /// Rejected document format.
        format: SourceFormat,
    },
    /// Two of the document, converter buffer, and analysis buffer share an identity.
    #[error("source buffers must have distinct file IDs: {id:?}")]
    DuplicateFileId {
        /// Identity assigned to more than one buffer.
        id: FileId,
    },
    /// A converter identity field contains no non-whitespace characters.
    #[error("converter identity field {field} must not be blank")]
    IncompleteConverterIdentity {
        /// Name of the rejected converter field.
        field: &'static str,
    },
    /// More than one import realm uses the same identifier.
    #[error("duplicate import realm ID {id}")]
    DuplicateImportRealm {
        /// Repeated realm identifier.
        id: u32,
    },
    /// An import realm references a parent absent from the projection.
    #[error("import realm {id} references missing parent {parent}")]
    MissingImportRealmParent {
        /// Realm whose parent is missing.
        id: u32,
        /// Missing parent identifier.
        parent: u32,
    },
    /// Following import realm parents reaches a cycle.
    #[error("import realm parent cycle at {id}")]
    ImportRealmCycle {
        /// Identifier in the cycle.
        id: u32,
    },
    /// An editable region references a realm absent from the projection.
    #[error("editable region {region} references missing import realm {id}")]
    MissingRegionImportRealm {
        /// Zero-based position in the editable region list.
        region: usize,
        /// Missing realm identifier.
        id: u32,
    },
    /// A realm span cannot address the editable document.
    #[error("invalid source span for import realm {id}: {source}")]
    InvalidImportRealmSpan {
        /// Realm whose source span was rejected.
        id: u32,
        /// Source validation failure, including the rejected span or identity.
        #[source]
        source: SourceError,
    },
    /// An editable region span cannot address the editable document.
    #[error("invalid source span for editable region {region}: {source}")]
    InvalidEditableRegionSpan {
        /// Zero-based position in the editable region list.
        region: usize,
        /// Source validation failure, including the rejected span or identity.
        #[source]
        source: SourceError,
    },
    /// Editable regions cover common bytes in the original document.
    #[error("editable regions {first} and {second} overlap")]
    OverlappingEditableRegions {
        /// Zero-based position of one region in the supplied list.
        first: usize,
        /// Zero-based position of the other region in the supplied list.
        second: usize,
    },
}

/// One module with an immutable, validated preparation of its source.
#[derive(Clone, Debug)]
pub struct SourceInput {
    prepared: PreparedSource,
    /// Compiler identity of the module represented by the input.
    pub module_id: ModuleId,
    /// Explicit output adapter constraint. Macro imports select it when absent.
    pub framework: Option<AdapterKind>,
    /// Host-allocated stylesheet modules inserted at a verified macro import.
    pub stylesheet_imports: Vec<String>,
    /// Host-allocated JavaScript module exporting this module's CSS value.
    pub module_css_import: Option<String>,
}

impl SourceInput {
    /// Takes a module identity and validated source, with no explicit adapter or CSS imports.
    pub fn new(module_id: ModuleId, prepared: PreparedSource) -> Self {
        Self {
            module_id,
            prepared,
            framework: None,
            stylesheet_imports: Vec::new(),
            module_css_import: None,
        }
    }

    /// Creates a direct input whose document format is inferred from the parser dialect.
    /// The module identity and grammar are supplied by the host, independent of filenames.
    pub fn normal(
        code: Arc<SourceFile>,
        module_id: ModuleId,
        language: ScriptLanguage,
        module_kind: ScriptModuleKind,
    ) -> Self {
        Self::new(
            module_id,
            PreparedSource::Direct(DirectSource::script(
                code,
                ScriptSyntax::new(language, module_kind),
            )),
        )
    }

    /// Returns the editable document, including its original format and upstream map.
    pub fn document(&self) -> &SourceDocument {
        match &self.prepared {
            PreparedSource::Direct(source) => source.document(),
            PreparedSource::Projected(source) => source.document(),
        }
    }

    /// Returns the direct script or converter buffer supplied to the parser pipeline.
    pub fn parser_source(&self) -> &Arc<SourceFile> {
        match &self.prepared {
            PreparedSource::Direct(source) => source.document().file(),
            PreparedSource::Projected(source) => source.projection().code(),
        }
    }

    /// Returns the grammar of the buffer supplied to the parser pipeline.
    pub fn syntax(&self) -> ScriptSyntax {
        match &self.prepared {
            PreparedSource::Direct(source) => source.syntax(),
            PreparedSource::Projected(source) => source.projection().syntax(),
        }
    }

    /// Returns the converter metadata when this input is projected.
    pub fn projection(&self) -> Option<&SourceProjection> {
        self.projected().map(ProjectedSource::projection)
    }

    /// Returns the validated pairing of document and projection, when present.
    pub fn projected(&self) -> Option<&ProjectedSource> {
        match &self.prepared {
            PreparedSource::Direct(_) => None,
            PreparedSource::Projected(source) => Some(source),
        }
    }

    /// Returns the immutable source preparation owned by this input.
    pub fn prepared(&self) -> &PreparedSource {
        &self.prepared
    }

    /// Includes every source, identity, grammar, format, map, converter field, realm,
    /// region, adapter constraint, and CSS import in the cache revision.
    /// Debug representations contain values only, so equal revisions identify inputs
    /// that are interchangeable for analysis.
    pub fn revision(&self) -> Fingerprint {
        Fingerprint::of(format!("{self:?}"))
    }
}

/// A validated direct script or component projection ready for analysis.
#[derive(Clone, Debug)]
pub enum PreparedSource {
    /// A script whose parser buffer is also its editable document.
    Direct(DirectSource),
    /// A component document paired with its converter buffer and metadata.
    Projected(ProjectedSource),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u32, text: &str) -> Arc<SourceFile> {
        Arc::new(SourceFile::new(FileId(id), None, None, text).unwrap())
    }

    #[test]
    fn normal_infers_only_document_format_and_keeps_explicit_grammar() {
        for language in [
            ScriptLanguage::Js,
            ScriptLanguage::Jsx,
            ScriptLanguage::Ts,
            ScriptLanguage::Tsx,
            ScriptLanguage::Dts,
        ] {
            let source = file(0, "const x = 1;");
            let input = SourceInput::normal(
                source.clone(),
                "opaque".into(),
                language,
                ScriptModuleKind::CommonJs,
            );
            assert_eq!(input.document().format(), SourceFormat::script(language));
            assert_eq!(input.syntax().language(), language);
            assert_eq!(input.syntax().module_kind(), ScriptModuleKind::CommonJs);
            assert!(Arc::ptr_eq(input.document().file(), &source));
            assert!(Arc::ptr_eq(input.parser_source(), &source));
            assert!(input.projection().is_none());
            assert!(input.projected().is_none());
            assert!(matches!(input.prepared(), PreparedSource::Direct(_)));
            assert!(input.document().upstream_map().is_none());
            assert!(input.framework.is_none());
            assert_eq!(input.stylesheet_imports, [] as [String; 0]);
            assert!(input.module_css_import.is_none());
        }
    }
}
