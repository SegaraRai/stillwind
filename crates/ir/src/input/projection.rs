//! Component projections with validated identities, spans, and import relationships.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::{FileId, ProjectionMap, SourceError, SourceFile, Span};

use super::{ScriptSyntax, SourceDocument, SourceInputError};
use crate::AdapterKind;

/// A component document paired with metadata validated against its original bytes.
#[derive(Clone, Debug)]
pub struct ProjectedSource {
    document: SourceDocument,
    projection: SourceProjection,
    adapter: AdapterKind,
}

impl ProjectedSource {
    /// Takes a component document and projection with distinct document, code, and analysis IDs.
    /// Every realm and editable region must be a valid UTF-8 byte slice of the document.
    /// Parents declare import visibility and may occupy separate source ranges.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-component format, reused buffer identity, or invalid
    /// original source span. The projection has already validated import relationships
    /// and region interiors. Map restoration is deferred to the analyzer.
    pub fn try_new(
        document: SourceDocument,
        projection: SourceProjection,
    ) -> Result<Self, SourceInputError> {
        let adapter = document.format().projection_adapter().ok_or(
            SourceInputError::InvalidProjectedFormat {
                format: document.format(),
            },
        )?;
        validate_file_ids(&[
            document.file().id(),
            projection.code().id(),
            projection.analysis_id(),
        ])?;
        validate_metadata_spans(
            projection.import_realms(),
            projection.editable_regions(),
            Some(document.file()),
        )?;
        Ok(Self {
            document,
            projection,
            adapter,
        })
    }

    /// Returns the editable component document.
    pub fn document(&self) -> &SourceDocument {
        &self.document
    }

    /// Returns the immutable converter buffer and metadata validated for this document.
    pub fn projection(&self) -> &SourceProjection {
        &self.projection
    }

    /// Returns the normalization adapter determined by the component document format.
    pub const fn adapter(&self) -> AdapterKind {
        self.adapter
    }
}

/// Converter output whose identities and import relationships are internally consistent.
#[derive(Clone, Debug)]
pub struct SourceProjection {
    code: Arc<SourceFile>,
    syntax: ScriptSyntax,
    map: ProjectionMap,
    converter: ConverterIdentity,
    analysis_id: FileId,
    import_realms: Vec<ImportRealm>,
    editable_regions: Vec<ProjectionRegion>,
}

impl SourceProjection {
    /// Takes converter bytes, parser grammar, map, identity, analysis ID, realms, and regions.
    /// Realm and region spans use original-document UTF-8 byte coordinates. The
    /// document's bounds and scalar boundaries are checked by [`ProjectedSource::try_new`].
    ///
    /// # Errors
    ///
    /// Returns an error for blank converter fields, equal code and analysis IDs,
    /// duplicate realm IDs, missing parents or region realms, parent cycles, reversed
    /// spans, or overlapping region interiors. Adjacent and empty regions are accepted.
    /// The map is retained without restoration checks.
    pub fn try_new(
        code: Arc<SourceFile>,
        syntax: ScriptSyntax,
        map: ProjectionMap,
        converter: ConverterIdentity,
        analysis_id: FileId,
        import_realms: Vec<ImportRealm>,
        editable_regions: Vec<ProjectionRegion>,
    ) -> Result<Self, SourceInputError> {
        validate_converter(&converter)?;
        validate_file_ids(&[code.id(), analysis_id])?;
        validate_import_metadata(&import_realms, &editable_regions)?;
        Ok(Self {
            code,
            syntax,
            map,
            converter,
            analysis_id,
            import_realms,
            editable_regions,
        })
    }

    /// Returns the converter buffer supplied to the parser pipeline.
    pub fn code(&self) -> &Arc<SourceFile> {
        &self.code
    }

    /// Returns the grammar of the converter buffer.
    pub const fn syntax(&self) -> ScriptSyntax {
        self.syntax
    }

    /// Returns the generated-to-original map in the converter's transport format.
    pub fn map(&self) -> &ProjectionMap {
        &self.map
    }

    /// Returns the converter identity included in the input revision.
    pub fn converter(&self) -> &ConverterIdentity {
        &self.converter
    }

    /// Returns the identity reserved for normalized analysis spans.
    pub const fn analysis_id(&self) -> FileId {
        self.analysis_id
    }

    /// Returns the original document's import scopes in host-supplied order.
    pub fn import_realms(&self) -> &[ImportRealm] {
        &self.import_realms
    }

    /// Returns `id`'s realm followed by its parents in order of import visibility.
    /// An unknown ID produces an empty iterator. Validated parent chains terminate.
    pub fn realm_chain(&self, id: u32) -> impl Iterator<Item = &ImportRealm> {
        std::iter::successors(
            self.import_realms.iter().find(|realm| realm.id == id),
            |realm| {
                realm
                    .parent
                    .and_then(|parent| self.import_realms.iter().find(|realm| realm.id == parent))
            },
        )
    }

    /// Returns the editable original regions in host-supplied order.
    pub fn editable_regions(&self) -> &[ProjectionRegion] {
        &self.editable_regions
    }
}

/// Checks the supplied buffer identities and returns an error for the first duplicate.
fn validate_file_ids(ids: &[FileId]) -> Result<(), SourceInputError> {
    let mut seen = BTreeSet::new();
    for &id in ids {
        if !seen.insert(id) {
            return Err(SourceInputError::DuplicateFileId { id });
        }
    }
    Ok(())
}

/// Checks each converter identity field and returns an error for the first blank field.
fn validate_converter(converter: &ConverterIdentity) -> Result<(), SourceInputError> {
    for (field, value) in [
        ("name", &converter.name),
        ("version", &converter.version),
        ("options_fingerprint", &converter.options_fingerprint),
    ] {
        if value.trim().is_empty() {
            return Err(SourceInputError::IncompleteConverterIdentity { field });
        }
    }
    Ok(())
}

/// Checks an original span against `document` when present, or its ordering otherwise.
/// Returns the source error without changing the span or document.
fn validate_original_span(span: Span, document: Option<&SourceFile>) -> Result<(), SourceError> {
    if let Some(document) = document {
        document.slice(span).map(|_| ())
    } else if span.start > span.end {
        Err(SourceError::InvalidSpan(span))
    } else {
        Ok(())
    }
}

/// Checks every metadata span against `document`, or only its ordering when absent.
/// Returns the first source failure together with its realm ID or region index.
fn validate_metadata_spans(
    realms: &[ImportRealm],
    regions: &[ProjectionRegion],
    document: Option<&SourceFile>,
) -> Result<(), SourceInputError> {
    for realm in realms {
        validate_original_span(realm.source, document).map_err(|source| {
            SourceInputError::InvalidImportRealmSpan {
                id: realm.id,
                source,
            }
        })?;
    }
    for (region, value) in regions.iter().enumerate() {
        validate_original_span(value.source, document)
            .map_err(|source| SourceInputError::InvalidEditableRegionSpan { region, source })?;
    }
    Ok(())
}

/// Checks realm identifiers, parent chains, region references, span ordering, and
/// overlapping region interiors. Parents declare import visibility independently of
/// spatial containment. Returns the first invalid relationship without changing metadata.
fn validate_import_metadata(
    realms: &[ImportRealm],
    regions: &[ProjectionRegion],
) -> Result<(), SourceInputError> {
    validate_metadata_spans(realms, regions, None)?;
    let mut parents = BTreeMap::new();
    for realm in realms {
        if parents.insert(realm.id, realm.parent).is_some() {
            return Err(SourceInputError::DuplicateImportRealm { id: realm.id });
        }
    }
    for realm in realms {
        if let Some(parent) = realm.parent
            && !parents.contains_key(&parent)
        {
            return Err(SourceInputError::MissingImportRealmParent {
                id: realm.id,
                parent,
            });
        }
    }
    let mut complete = BTreeSet::new();
    for realm in realms {
        let mut chain = BTreeSet::new();
        let mut current = Some(realm.id);
        while let Some(id) = current {
            if complete.contains(&id) {
                break;
            }
            if !chain.insert(id) {
                return Err(SourceInputError::ImportRealmCycle { id });
            }
            current = parents.get(&id).copied().flatten();
        }
        complete.extend(chain);
    }
    for (region, value) in regions.iter().enumerate() {
        if !parents.contains_key(&value.import_realm) {
            return Err(SourceInputError::MissingRegionImportRealm {
                region,
                id: value.import_realm,
            });
        }
    }
    let mut ordered: Vec<_> = regions
        .iter()
        .enumerate()
        .filter(|(_, region)| !region.source.is_empty())
        .collect();
    ordered.sort_by_key(|(_, region)| (region.source.file, region.source.start, region.source.end));
    for pair in ordered.windows(2) {
        let (first, left) = pair[0];
        let (second, right) = pair[1];
        if left.source.file == right.source.file && left.source.end > right.source.start {
            return Err(SourceInputError::OverlappingEditableRegions { first, second });
        }
    }
    Ok(())
}

/// Identifies the converter that produced a projection, for cache identity.
///
/// `options_fingerprint` must change whenever converter settings change the produced
/// mapping; otherwise a cached artifact can outlive the settings that justified it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterIdentity {
    /// Converter package or tool name.
    pub name: LeanStr,
    /// Converter version string.
    pub version: LeanStr,
    /// Fingerprint of the converter settings that produced the mapping.
    pub options_fingerprint: LeanStr,
}

/// An import scope of a converted framework file.
///
/// A realm is the set of imports visible at one place in the original file: the script
/// block of a Vue SFC, or one expression of an Astro template. Bindings resolve to the
/// realm named by their editable region. `parent` declares import visibility and may
/// reference a script block in a separate source range.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRealm {
    /// Host-chosen identifier, referenced by [`ProjectionRegion::import_realm`].
    pub id: u32,
    /// Span of the realm in the original source.
    pub source: Span,
    /// Parent scope whose imports are visible in this realm, regardless of source range.
    pub parent: Option<u32>,
}

/// A region of the original file that was converted into JavaScript.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::super::{ScriptLanguage, ScriptModuleKind, SourceFormat};
    use super::*;

    fn file(id: u32, text: &str) -> Arc<SourceFile> {
        Arc::new(SourceFile::new(FileId(id), None, None, text).unwrap())
    }

    fn syntax() -> ScriptSyntax {
        ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Module)
    }

    fn converter() -> ConverterIdentity {
        ConverterIdentity {
            name: "converter".into(),
            version: "1".into(),
            options_fingerprint: "options".into(),
        }
    }

    fn realm(id: u32, start: u32, end: u32, parent: Option<u32>) -> ImportRealm {
        ImportRealm {
            id,
            source: Span::new(FileId(1), start, end),
            parent,
        }
    }

    fn region(id: u32, start: u32, end: u32) -> ProjectionRegion {
        ProjectionRegion {
            source: Span::new(FileId(1), start, end),
            import_realm: id,
            attribute_quote: None,
        }
    }

    fn projection(
        realms: Vec<ImportRealm>,
        regions: Vec<ProjectionRegion>,
    ) -> Result<SourceProjection, SourceInputError> {
        SourceProjection::try_new(
            file(2, "const x = 1;"),
            syntax(),
            ProjectionMap::Volar {
                mappings: Vec::new(),
            },
            converter(),
            FileId(3),
            realms,
            regions,
        )
    }

    fn projected(
        document: Arc<SourceFile>,
        realms: Vec<ImportRealm>,
        regions: Vec<ProjectionRegion>,
    ) -> Result<ProjectedSource, SourceInputError> {
        ProjectedSource::try_new(
            SourceDocument::new(document, SourceFormat::Svelte, None),
            projection(realms, regions)?,
        )
    }

    #[test]
    fn projected_sources_require_components_and_derive_their_adapter() {
        for (format, adapter) in [
            (SourceFormat::Svelte, AdapterKind::Svelte),
            (SourceFormat::Astro, AdapterKind::Astro),
            (SourceFormat::Vue, AdapterKind::Vue),
        ] {
            let source = ProjectedSource::try_new(
                SourceDocument::new(file(1, "<div/>"), format, None),
                projection(vec![], vec![]).unwrap(),
            )
            .unwrap();
            assert_eq!(source.adapter(), adapter);
            assert_eq!(source.document().file().id(), FileId(1));
            assert_eq!(source.projection().code().id(), FileId(2));
            assert_eq!(source.projection().syntax(), syntax());
            let projection = source.projection();
            assert_eq!(projection.analysis_id(), FileId(3));
            assert_eq!(projection.converter(), &converter());
            assert_eq!(projection.import_realms(), []);
            assert_eq!(projection.editable_regions(), []);
            assert!(
                matches!(projection.map(), ProjectionMap::Volar { mappings } if mappings.is_empty())
            );
        }
        for format in [SourceFormat::Script, SourceFormat::Jsx] {
            assert!(matches!(
                ProjectedSource::try_new(
                    SourceDocument::new(file(1, ""), format, None),
                    projection(vec![], vec![]).unwrap(),
                ),
                Err(SourceInputError::InvalidProjectedFormat { format: actual }) if actual == format
            ));
        }
    }

    #[test]
    fn buffer_identities_must_be_distinct_but_zero_is_valid() {
        for (code_id, analysis_id) in [(0, 0), (2, 2)] {
            assert!(matches!(
                SourceProjection::try_new(
                    file(code_id, ""), syntax(), ProjectionMap::Volar { mappings: vec![] },
                    converter(), FileId(analysis_id), vec![], vec![],
                ),
                Err(SourceInputError::DuplicateFileId { id }) if id == FileId(code_id)
            ));
        }
        for id in [2, 3] {
            assert!(matches!(
                projected(file(id, ""), vec![], vec![]),
                Err(SourceInputError::DuplicateFileId { id: actual }) if actual == FileId(id)
            ));
        }
        assert!(projected(file(0, ""), vec![], vec![]).is_ok());
    }

    #[test]
    fn converter_identity_requires_each_field_to_be_nonblank() {
        for field in ["name", "version", "options_fingerprint"] {
            for blank in ["", " \t\n"] {
                let mut identity = converter();
                match field {
                    "name" => {
                        identity.name = blank.into();
                    }
                    "version" => {
                        identity.version = blank.into();
                    }
                    _ => {
                        identity.options_fingerprint = blank.into();
                    }
                }
                assert!(matches!(
                    SourceProjection::try_new(
                        file(2, ""), syntax(), ProjectionMap::Volar { mappings: vec![] },
                        identity, FileId(3), vec![], vec![],
                    ),
                    Err(SourceInputError::IncompleteConverterIdentity { field: actual }) if actual == field
                ));
            }
        }
    }

    #[test]
    fn realm_graph_rejects_duplicate_ids_and_missing_references() {
        assert!(matches!(
            projection(vec![realm(7, 0, 1, None), realm(7, 2, 3, None)], vec![]),
            Err(SourceInputError::DuplicateImportRealm { id: 7 })
        ));
        assert!(matches!(
            projection(vec![realm(7, 0, 1, Some(8))], vec![]),
            Err(SourceInputError::MissingImportRealmParent { id: 7, parent: 8 })
        ));
        assert!(matches!(
            projection(vec![realm(7, 0, 1, None)], vec![region(8, 2, 3)]),
            Err(SourceInputError::MissingRegionImportRealm { region: 0, id: 8 })
        ));
    }

    #[test]
    fn realm_graph_rejects_cycles_even_without_editable_regions() {
        for realms in [
            vec![realm(7, 0, 1, Some(7))],
            vec![realm(7, 0, 1, Some(8)), realm(8, 2, 3, Some(7))],
            vec![
                realm(6, 4, 5, None),
                realm(7, 0, 1, Some(8)),
                realm(8, 2, 3, Some(9)),
                realm(9, 4, 5, Some(7)),
            ],
        ] {
            assert!(matches!(
                projection(realms, vec![]),
                Err(SourceInputError::ImportRealmCycle { .. })
            ));
        }
    }

    #[test]
    fn realms_and_regions_may_occupy_separate_ranges() {
        let source = projected(
            file(1, "module instance template"),
            vec![realm(2, 7, 15, Some(1)), realm(1, 0, 6, None)],
            vec![region(2, 16, 24)],
        )
        .unwrap();
        assert_eq!(
            source
                .projection()
                .realm_chain(2)
                .map(|realm| realm.id)
                .collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(
            source
                .projection()
                .realm_chain(1)
                .map(|realm| realm.id)
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(source.projection().realm_chain(99).count(), 0);
        assert!(
            projected(
                file(1, ""),
                vec![realm(1, 0, 0, None), realm(2, 0, 0, None)],
                vec![],
            )
            .is_ok()
        );
    }

    #[test]
    fn all_realm_and_region_spans_require_document_ownership_and_scalar_bounds() {
        for span in [
            Span::new(FileId(2), 0, 1),
            Span::new(FileId(1), 4, 7),
            Span::new(FileId(1), 2, 3),
            Span::new(FileId(1), 1, 2),
        ] {
            let result = projected(
                file(1, "a😀b"),
                vec![ImportRealm {
                    id: 7,
                    source: span,
                    parent: None,
                }],
                vec![],
            );
            assert!(matches!(
                result,
                Err(SourceInputError::InvalidImportRealmSpan { id: 7, .. })
            ));
            let result = projected(
                file(1, "a😀b"),
                vec![realm(7, 0, 1, None)],
                vec![ProjectionRegion {
                    source: span,
                    ..region(7, 0, 1)
                }],
            );
            assert!(matches!(
                result,
                Err(SourceInputError::InvalidEditableRegionSpan { region: 0, .. })
            ));
        }
        assert!(
            projected(
                file(1, "a😀b"),
                vec![realm(7, 1, 5, None)],
                vec![region(7, 5, 6), region(7, 6, 6)],
            )
            .is_ok()
        );
    }

    #[test]
    fn reversed_spans_fail_before_a_document_is_attached() {
        assert!(matches!(
            projection(vec![realm(7, 3, 2, None)], vec![]),
            Err(SourceInputError::InvalidImportRealmSpan {
                source: SourceError::InvalidSpan(_),
                ..
            })
        ));
        assert!(matches!(
            projection(vec![realm(7, 0, 1, None)], vec![region(7, 3, 2)]),
            Err(SourceInputError::InvalidEditableRegionSpan {
                source: SourceError::InvalidSpan(_),
                ..
            })
        ));
    }

    #[test]
    fn editable_regions_reject_overlapping_interiors_in_any_order() {
        for regions in [
            vec![region(7, 0, 4), region(7, 3, 5)],
            vec![region(7, 3, 5), region(7, 0, 4)],
            vec![region(7, 0, 4), region(7, 0, 4)],
            vec![region(7, 0, 5), region(7, 1, 1), region(7, 2, 3)],
        ] {
            assert!(matches!(
                projection(vec![realm(7, 0, 5, None)], regions),
                Err(SourceInputError::OverlappingEditableRegions { .. })
            ));
        }
        assert!(
            projection(
                vec![realm(7, 0, 6, None)],
                vec![
                    region(7, 2, 4),
                    region(7, 0, 2),
                    region(7, 4, 6),
                    region(7, 1, 1),
                    region(7, 1, 1)
                ],
            )
            .is_ok()
        );
    }

    #[test]
    fn map_and_quote_context_are_deferred_to_consumers() {
        let projection = SourceProjection::try_new(
            file(2, "generated"),
            syntax(),
            ProjectionMap::V3 {
                json: "invalid map".into(),
                source: "original".into(),
            },
            converter(),
            FileId(3),
            vec![realm(7, 0, 1, None)],
            vec![ProjectionRegion {
                attribute_quote: Some(AttributeQuote::Double),
                ..region(7, 2, 3)
            }],
        )
        .unwrap();
        assert!(
            ProjectedSource::try_new(
                SourceDocument::new(
                    file(1, "abc"),
                    SourceFormat::Vue,
                    Some("invalid upstream map".into())
                ),
                projection,
            )
            .is_ok()
        );
    }
}
