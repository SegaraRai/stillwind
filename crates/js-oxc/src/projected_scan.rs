//! Scanning a converter's shadow module and answering in original coordinates.
//!
//! The pipeline is: build a [`SourceFile`] for the generated text, normalize it against
//! the projection, parse the normalized source, map the IR back to the original file,
//! then rewrite every diagnostic label, lint label, module specifier, asset record and
//! exported symbolic value into original spans. Anything that cannot be mapped is
//! reported rather than left in generated coordinates.
//!
//! Checker-fact application for projected inputs is not implemented, so
//! [`ParsedJavaScript::type_probes`] is cleared and the analysis is intentionally
//! fact-free.

use std::collections::BTreeMap;

use stillwind_ir::SourceInput;
use stillwind_semantic::{Candidates, ExportedValue, IdentityId, SymbolicValue};
use stillwind_source::{ProjectionError, SourceFile, SourceProvenance, Span};

use crate::normalization::normalize_with_language;
use crate::{NormalizedProjection, ParsedJavaScript, parse_with_language};

/// Parses a projected input and returns IR plus the provenance it collected.
///
/// The converter identity must be complete (name, version, options fingerprint),
/// because the cache key of the resulting artifact depends on it. Diagnostics whose
/// labels cannot be mapped point at the start of the original file instead of at a
/// generated offset.
pub(crate) fn parse_projected(
    input: &SourceInput,
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
    let mut parsed = parse_with_language(&normalized.source, &[], &BTreeMap::new(), input.language);
    let (ir, provenance) = normalized.original_ir(&parsed.module)?;
    parsed.module = ir;
    parsed.type_probes.clear(); // Checker projections are a separate, deferred contract.
    let map_span = |span| {
        normalized
            .original_span(span)
            .map(|provenance| provenance.original)
    };
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
        if let Ok(span) = map_span(Span::new(
            normalized.source.id(),
            specifier.start,
            specifier.end,
        )) {
            specifier.start = span.start.0;
            specifier.end = span.end.0;
            mapped_specifiers.push(specifier);
        }
    }
    parsed.summary.module_specifiers = mapped_specifiers;
    for asset in &mut parsed.summary.assets {
        map_identity(&mut asset.identity, &normalized)?;
        let span = map_span(Span::new(
            normalized.source.id(),
            asset.call_start,
            asset.call_end,
        ))?;
        asset.call_start = span.start.0;
        asset.call_end = span.end.0;
    }
    for value in parsed.summary.exports.values_mut() {
        match value {
            ExportedValue::Refs(alternatives) => {
                for alternative in alternatives {
                    map_identity(&mut alternative.reference, &normalized)?;
                    if let Some(scopes) = &mut alternative.permitted_scopes {
                        map_symbolic(scopes, &normalized)?;
                    }
                }
            }
            ExportedValue::Styles(value)
            | ExportedValue::Scopes(value)
            | ExportedValue::Animations(value)
            | ExportedValue::ViewTransitions(value) => {
                map_symbolic(value, &normalized)?;
            }
        }
    }
    Ok((parsed, provenance))
}

/// Rewrites an identity's declaration span into the original file.
///
/// The module path and declaration offsets in the identity are rewritten together.
/// The resulting key names the original declaration rather than the generated copy.
fn map_identity(
    identity: &mut IdentityId,
    normalized: &NormalizedProjection<'_>,
) -> Result<(), ProjectionError> {
    let span = normalized
        .original_span(Span::new(
            normalized.source.id(),
            identity.start,
            identity.end,
        ))?
        .original;
    identity.start = span.start.0;
    identity.end = span.end.0;
    Ok(())
}

/// Rewrites every finite candidate of a symbolic value into original spans.
///
/// A universe value needs no mapping, because it holds no identities.
fn map_symbolic(
    value: &mut SymbolicValue,
    normalized: &NormalizedProjection<'_>,
) -> Result<(), ProjectionError> {
    if let Candidates::Finite(candidates) = &mut value.candidates {
        *candidates = candidates
            .iter()
            .cloned()
            .map(|mut identity| {
                map_identity(&mut identity, normalized)?;
                Ok(identity)
            })
            .collect::<Result<_, ProjectionError>>()?;
    }
    Ok(())
}
