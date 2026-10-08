//! Scans prepared sources and transports summaries for checker queries.

use std::collections::BTreeMap;

use napi::Error;
use napi_derive::napi;

use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, DiagnosticBatch, LintSettings, Reporter,
};
use stillwind_ir::{CheckerRole, ModuleScan, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_source::{CoordinateIndex, FileId, ModuleId, SourceFile, SourceRegistry};

use super::BindingApiError;
use crate::output::{NativeDiagnosticBatch, NativeScanResult};
use crate::semantic::{
    NativeCheckerBinding, NativeCheckerSpan, NativeModuleSummary, NativeTypeProbe,
};
use crate::source_input::{
    HostPreparedSource, NativePreparedSource, NativeScriptLanguage, NativeScriptModuleKind,
};

/// Scans plain or projected input before dependency traversal and checker queries.
/// Ready summaries use original UTF-8 coordinates. Fatal syntax and projection
/// failures return resolved reports for the original document.
///
/// # Errors
///
/// Rejects malformed host input and failures to resolve diagnostic locations.
#[napi(strict)]
pub fn scan_source(source: NativePreparedSource) -> napi::Result<NativeScanResult> {
    let source = HostPreparedSource::try_from(source)?;
    let module = source.module_id().clone();
    let mut registry = SourceRegistry::default();
    Ok(scan_prepared(&source, &mut registry)?.into_binding(&module))
}

/// Internal scan outcome that retains typed errors until the host publishes reports.
pub(crate) enum SourceScan {
    Ready(Box<NativeModuleSummary>),
    Failed {
        scan: Box<ModuleScan>,
        batch: DiagnosticBatch,
    },
}

impl SourceScan {
    pub(crate) fn into_binding(self, module: &ModuleId) -> NativeScanResult {
        match self {
            Self::Ready(summary) => NativeScanResult::Ready { summary: *summary },
            Self::Failed { batch, .. } => NativeScanResult::Failed {
                batch: NativeDiagnosticBatch::new(module, &batch),
            },
        }
    }
}

/// Scans a prepared source using the caller's registry and resolves fatal reports.
pub(crate) fn scan_prepared(
    source: &HostPreparedSource,
    registry: &mut SourceRegistry,
) -> napi::Result<SourceScan> {
    let mut scan = OxcSourceAnalyzer.scan_input(source.input(registry)?);
    let fatal = |error: &CompileError| {
        error.diagnostic.is_projection_failure()
            || matches!(error.diagnostic, CompileDiagnostic::JavaScriptParse)
            || matches!(
                error.diagnostic,
                CompileDiagnostic::UnmappedMacroReference { .. }
                    | CompileDiagnostic::CheckerTargetUnavailable { .. }
            )
    };
    if scan.diagnostics.iter().any(fatal) {
        scan.diagnostics.retain(fatal);
        let batch = Reporter::collect(&scan.diagnostics, &[], registry, LintSettings::default())
            .map_err(BindingApiError::from)?;
        return Ok(SourceScan::Failed {
            scan: Box::new(scan),
            batch,
        });
    }
    let count = |length| u32::try_from(length).map_err(|_| Error::from_reason("too many imports"));
    let mut result = NativeModuleSummary::new(
        scan.summary,
        count(scan.local_ir.imports.module_css.len())?,
        count(scan.local_ir.imports.macros.len())?,
        scan.type_probes
            .iter()
            .map(|probe| NativeTypeProbe {
                recognition_only: probe.recognition_only,
                start: probe.span.start.0,
                end: probe.span.end.0,
            })
            .collect(),
    );
    let index = CoordinateIndex::new(scan.input.parser_source());
    for checked in scan.checker_projection.spans {
        let mapped = checked.provenance;
        result.checker_spans.push(NativeCheckerSpan {
            role: match checked.role {
                CheckerRole::Expression => "expression",
                CheckerRole::Producer => "producer",
                CheckerRole::ModuleSpecifier => "moduleSpecifier",
            }
            .into(),
            start: mapped.original.start.0,
            end: mapped.original.end.0,
            projected_start: index
                .utf16(mapped.projected.start)
                .map_err(|e| Error::from_reason(e.to_string()))?
                .0,
            projected_end: index
                .utf16(mapped.projected.end)
                .map_err(|e| Error::from_reason(e.to_string()))?
                .0,
        });
    }
    for binding in scan.checker_projection.bindings {
        result.checker_bindings.push(NativeCheckerBinding {
            name: binding.name,
            projected_start: index
                .utf16(binding.span.start)
                .map_err(|e| Error::from_reason(e.to_string()))?
                .0,
            projected_end: index
                .utf16(binding.span.end)
                .map_err(|e| Error::from_reason(e.to_string()))?
                .0,
        });
    }
    Ok(SourceScan::Ready(Box::new(result)))
}

/// Extracts the versioned cross-module summary for a source module.
///
/// # Errors
///
/// Returns invalid source coordinates.
pub fn summarize_source_input(
    source: &str,
    module_id: &ModuleId,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
) -> Result<NativeModuleSummary, BindingApiError> {
    let source = SourceFile::new(FileId(1), None, None, source)?;
    let parsed = parse(
        &source,
        module_id,
        &[],
        &BTreeMap::default(),
        language,
        module_kind,
    );
    let count = |length| {
        u32::try_from(length).map_err(|_| BindingApiError::Input("Too many imports".into()))
    };
    Ok(NativeModuleSummary::new(
        parsed.summary,
        count(parsed.module.imports.module_css.len())?,
        count(parsed.module.imports.macros.len())?,
        parsed
            .type_probes
            .iter()
            .map(|span| NativeTypeProbe {
                recognition_only: false,
                start: span.start.0,
                end: span.end.0,
            })
            .chain(parsed.use_probes.iter().map(|span| NativeTypeProbe {
                recognition_only: true,
                start: span.start.0,
                end: span.end.0,
            }))
            .collect(),
    ))
}

/// Returns the versioned Stillwind module summary used for cross-module values.
///
/// # Errors
///
/// Returns a JavaScript error for invalid source coordinates.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn summarize_source(
    source: String,
    module_id: String,
    language: NativeScriptLanguage,
    module_kind: NativeScriptModuleKind,
) -> napi::Result<NativeModuleSummary> {
    summarize_source_input(
        &source,
        &ModuleId::from(module_id),
        language.into(),
        module_kind.into(),
    )
    .map_err(|error| Error::from_reason(error.to_string()))
}
