//! Scanning a converter's shadow module and answering in original coordinates.
//!
//! The pipeline is: build a [`SourceFile`] for the generated text, normalize it against
//! the projection, parse the normalized source, map the IR back to the original file,
//! then rewrite every diagnostic label, lint label, module specifier, asset record and
//! exported symbolic value into original spans. Anything that cannot be mapped is
//! reported rather than left in generated coordinates.
use std::collections::BTreeMap;

use stillwind_ir::SourceInput;
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFact};
use stillwind_source::{ProjectionError, SourceFile, SourceProvenance, Span};

use crate::ParsedJavaScript;
use crate::entry::parse_with_coordinates;
use crate::normalization::normalize_with_language;

/// Parses a projected input and returns IR plus the provenance it collected.
///
/// The converter identity must be complete (name, version, options fingerprint),
/// because the cache key of the resulting artifact depends on it. Diagnostics whose
/// labels cannot be mapped point at the start of the original file instead of at a
/// generated offset.
pub(crate) fn parse_projected(
    input: &SourceInput,
    facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> Result<(ParsedJavaScript, Vec<SourceProvenance>), ProjectionError> {
    let projection = input
        .projection
        .as_ref()
        .ok_or_else(|| ProjectionError::Map("missing projection".into()))?;
    let framework = input.framework.ok_or_else(|| {
        ProjectionError::Map("projected inputs require an explicit framework".into())
    })?;
    if projection.converter.name.is_empty()
        || projection.converter.version.is_empty()
        || projection.converter.options_fingerprint.is_empty()
    {
        return Err(ProjectionError::Map("incomplete converter identity".into()));
    }
    let generated = SourceFile::new(
        input.code.id(),
        projection.original.path(),
        input.code.text(),
    )?;
    let normalized = normalize_with_language(
        &generated,
        &projection.original,
        &projection.map,
        framework,
        projection.analysis_id,
        input.language,
    )?;
    let coordinates = |span| {
        normalized
            .original_span(span)
            .ok()
            .map(|mapped| mapped.original)
    };
    let mut parsed = parse_with_coordinates(
        &normalized.source,
        facts,
        assets,
        input.language,
        &projection.original,
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
    for diagnostic in &mut parsed.diagnostics {
        for label in &mut diagnostic.labels {
            label.span = map_span(label.span).unwrap_or(projection.original.full_span());
        }
    }
    for finding in &mut parsed.lint_findings {
        for label in &mut finding.labels {
            label.span = map_span(label.span).unwrap_or(projection.original.full_span());
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
