//! Scanning a converter's shadow module and answering in original coordinates.
//!
//! The pipeline normalizes the converter buffer against the projection, parses the
//! normalized source, maps the IR back to the original file,
//! then rewrite every diagnostic label, lint label, module specifier, asset record and
//! exported symbolic value into original spans. If required data cannot be mapped,
//! parsing returns an error or reports a diagnostic. Optional recognition probes
//! are omitted when their generated code has no corresponding original source.

use std::collections::BTreeMap;

use stillwind_ir::ProjectedSource;
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFact};
use stillwind_source::{ModuleId, ProjectionError, SourceProvenance, Span};

use crate::ParsedJavaScript;
use crate::entry::parse_with_coordinates;
use crate::normalization::normalize_with_language;

/// Parses a projected input and returns IR plus the provenance it collected.
///
/// The validated source supplies the component format and converter metadata.
/// `module_id` identifies the module for asset identities, and `facts` and `assets`
/// supply checker results in original coordinates. Mapping failures return an error.
/// Diagnostic labels that cannot be mapped use the original file span.
pub(crate) fn parse_projected(
    module_id: &ModuleId,
    projected: &ProjectedSource,
    facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> Result<(ParsedJavaScript, Vec<SourceProvenance>), ProjectionError> {
    let projection = projected.projection();
    let original = projected.document().file();
    let syntax = projection.syntax();
    let framework = projected.adapter();
    let generated = projection.code();
    let normalized = normalize_with_language(
        generated,
        original,
        projection.map(),
        framework,
        projection.analysis_id(),
        syntax.language(),
        syntax.module_kind(),
    )?;
    let coordinates = |span| {
        normalized
            .original_span(span)
            .ok()
            .map(|mapped| mapped.original)
    };
    let mut parsed = parse_with_coordinates(
        &normalized.source,
        module_id,
        facts,
        assets,
        syntax.language(),
        syntax.module_kind(),
        original,
        &coordinates,
    );
    let (ir, mut provenance) = normalized.original_ir(&parsed.module)?;
    parsed.module = ir;

    let map_span = |span| {
        normalized
            .original_span(span)
            .map(|provenance| provenance.original)
    };
    parsed.type_probes = parsed
        .type_probes
        .into_iter()
        .map(|span| {
            let mapped = normalized.original_span(span)?;
            provenance.push(mapped);
            Ok(mapped.original)
        })
        .collect::<Result<_, ProjectionError>>()?;
    parsed.use_probes = parsed
        .use_probes
        .into_iter()
        .filter_map(|span| {
            let mapped = normalized.original_span(span).ok()?;
            provenance.push(mapped);
            Some(mapped.original)
        })
        .collect();
    for diagnostic in &mut parsed.diagnostics {
        for label in &mut diagnostic.labels {
            label.span = map_span(label.span).unwrap_or(original.full_span());
        }
    }
    for finding in &mut parsed.lint_findings {
        for label in &mut finding.labels {
            label.span = map_span(label.span).unwrap_or(original.full_span());
        }
    }
    let mut mapped_specifiers = Vec::new();
    for mut specifier in parsed.summary.module_specifiers {
        if let Ok(mapped) = normalized.original_span(Span::new(
            normalized.source.id(),
            specifier.start,
            specifier.end,
        )) {
            provenance.push(mapped);
            specifier.start = mapped.original.start.0;
            specifier.end = mapped.original.end.0;
            mapped_specifiers.push(specifier);
        }
    }
    parsed.summary.module_specifiers = mapped_specifiers;
    for asset in &mut parsed.summary.assets {
        let span = map_span(Span::new(
            normalized.source.id(),
            asset.call_start,
            asset.call_end,
        ))?;
        asset.call_start = span.start.0;
        asset.call_end = span.end.0;
    }
    Ok((parsed, provenance))
}
