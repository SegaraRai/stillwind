//! Scanning a converter's shadow module and answering in original coordinates.
//!
//! The pipeline normalizes the converter buffer against the projection, parses the
//! normalized source, and maps the IR and summary back to the original file.
//! It also maps diagnostic and lint labels to original spans. If required data cannot be mapped,
//! parsing returns an error or reports a diagnostic. Optional recognition probes
//! are omitted when their generated code has no corresponding original source.

use std::collections::BTreeMap;

use stillwind_ir::{CheckerProjection, CheckerRole, ProjectedSource};
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{ModuleId, ProjectionError, SourceProvenance, Span};

use crate::ParsedJavaScript;
use crate::entry::parse_with_coordinates;
use crate::normalization::{NormalizedProjection, normalize_with_language};

/// Parses a projected input and returns original IR, provenance, and checker operations.
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
) -> Result<(ParsedJavaScript, Vec<SourceProvenance>, CheckerProjection), ProjectionError> {
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
    let mut checker = CheckerProjection {
        spans: Vec::new(),
        bindings: normalized.checker_bindings().to_vec(),
    };
    parsed.type_probes = parsed
        .type_probes
        .into_iter()
        .map(|span| {
            let target = normalized.checker_span(span, CheckerRole::Expression)?;
            let mapped = target.provenance;
            checker.spans.push(target);
            provenance.push(mapped);
            Ok(mapped.original)
        })
        .collect::<Result<_, ProjectionError>>()?;
    parsed.use_probes = parsed
        .use_probes
        .into_iter()
        .filter_map(|span| {
            let target = normalized
                .checker_span(span, CheckerRole::Expression)
                .ok()?;
            let mapped = target.provenance;
            checker.spans.push(target);
            provenance.push(mapped);
            Some(mapped.original)
        })
        .collect();
    let map_span = |span| {
        normalized
            .original_span(span)
            .map(|provenance| provenance.original)
    };
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
    map_summary(
        &normalized,
        &mut parsed.summary,
        &mut provenance,
        &mut checker,
    )?;
    Ok((parsed, provenance, checker))
}

fn map_summary(
    normalized: &NormalizedProjection<'_>,
    summary: &mut ModuleSummary,
    provenance: &mut Vec<SourceProvenance>,
    checker: &mut CheckerProjection,
) -> Result<(), ProjectionError> {
    let mut mapped_specifiers = Vec::new();
    for mut specifier in std::mem::take(&mut summary.module_specifiers) {
        if let Ok(mapped) = normalized.original_span(Span::new(
            normalized.source.id(),
            specifier.start,
            specifier.end,
        )) {
            checker.spans.push(normalized.checker_span(
                Span::new(normalized.source.id(), specifier.start, specifier.end),
                CheckerRole::ModuleSpecifier,
            )?);
            provenance.push(mapped);
            specifier.start = mapped.original.start.0;
            specifier.end = mapped.original.end.0;
            mapped_specifiers.push(specifier);
        }
    }
    summary.module_specifiers = mapped_specifiers;
    for asset in &mut summary.assets {
        let target = normalized.checker_span(
            Span::new(normalized.source.id(), asset.call_start, asset.call_end),
            CheckerRole::Producer,
        )?;
        checker.spans.push(target);
        let span = target.provenance.original;
        asset.call_start = span.start.0;
        asset.call_end = span.end.0;
    }
    let mut canonical = BTreeMap::new();
    for asset in std::mem::take(&mut summary.assets) {
        if let Some(previous) = canonical.insert(asset.identity.clone(), asset.clone())
            && previous != asset
        {
            return Err(ProjectionError::Map(
                "conflicting copies of an asset declaration".into(),
            ));
        }
    }
    summary.assets = canonical.into_values().collect();
    Ok(())
}
