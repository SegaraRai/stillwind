//! One-shot compile entry points, kept independent of Node-API.
//!
//! The `*_input` functions take ordinary Rust values so the same behaviour can be
//! tested without a JavaScript runtime. The `#[napi]` functions next to them only
//! convert arguments, so a JavaScript caller and a Rust test exercise one
//! implementation.

use std::collections::{BTreeMap, BTreeSet};

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{
    CompileResult, Compiler, CompilerOptions, Identifiers, ModuleCompiler, StyleResolver,
    TransformError, check_style, is_style_analysis_lint, render_transforms,
};
use stillwind_design_system::{DesignSystemError, DesignSystemSnapshot};
use stillwind_diagnostics::{
    AggregateError, CompileDiagnostic, CompileError, DiagnosticBatch, Reporter, ResolvedDiagnostic,
    SourceFailureContext, filter_lint_findings,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFacts};
use stillwind_source::{
    CoordinateIndex, CssSourceName, FileId, Fingerprint, ModuleId, SourceError, SourceFile,
    SourceRegistry, SourceRegistryError, SourceUri,
};

use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{BindingDiagnostic, BindingLintFinding, BindingOutput, BindingReport};
use crate::output::{ProjectBindingOutput, ProjectModuleOutput};
use crate::semantic::{NativeCheckerSpan, NativeModuleSummary, NativeTypeProbe};
use crate::source_input::{
    HostPreparedSource, NativePreparedSource, NativeScriptLanguage, NativeScriptModuleKind,
};

/// A Node binding input, setup, or diagnostic serialization error.
#[derive(Debug, thiserror::Error)]
pub enum BindingApiError {
    /// A host value was structurally invalid, such as a projection without its required
    /// companion field.
    #[error("invalid source input: {0}")]
    Input(String),
    /// A source buffer or coordinate was rejected.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// A span did not identify a registered original source revision.
    #[error("{0}")]
    Registry(#[from] SourceRegistryError),
    /// The common reporter rejected inconsistent locations or edits.
    #[error("{0}")]
    Aggregate(#[from] AggregateError),
    /// The design system CSS failed to load or link.
    #[error("{0}")]
    DesignSystem(#[from] DesignSystemError),
    /// Compiler options were rejected.
    #[error("{0}")]
    Options(#[from] CompileDiagnostic),
}

impl From<BindingApiError> for Error {
    fn from(error: BindingApiError) -> Self {
        Self::from_reason(error.to_string())
    }
}

/// Host supplied CSS text and its independent source identities.
pub struct CompileCssInput {
    /// Resolved CSS text supplied to the parser.
    pub content: String,
    /// Opaque identity used to resolve CSS imports and resources.
    pub name: CssSourceName,
    /// URI of the editable CSS document for diagnostics.
    pub uri: SourceUri,
    /// Exact source name written into generated CSS maps.
    pub source_map_source: Option<String>,
}

/// One plain script and CSS source supplied for a standalone compilation.
pub struct CompileSourceInput {
    /// Script text supplied to the parser.
    pub source: String,
    /// Opaque module identity used for references and generated names.
    pub module_id: ModuleId,
    /// URI of the editable script document for diagnostics.
    pub uri: SourceUri,
    /// Exact source name written into generated script maps.
    pub source_map_source: Option<String>,
    /// Script parser dialect selected by the host.
    pub language: ScriptLanguage,
    /// Script module grammar selected by the host.
    pub module_kind: ScriptModuleKind,
    /// CSS source and its identities.
    pub css: CompileCssInput,
}

/// Compiles one module with explicitly chosen compiler options.
///
/// # Errors
///
/// Returns an error when the options or source buffers are invalid.
pub fn compile_source_input(
    input: &CompileSourceInput,
    options: &CompilerOptions,
) -> Result<BindingOutput, BindingApiError> {
    options.validate()?;
    let mut sources = SourceRegistry::default();
    let source = sources.register(
        Some(input.uri.clone()),
        input.source_map_source.as_deref().map(Into::into),
        input.source.as_str(),
    )?;
    let theme = sources.register(
        Some(input.css.uri.clone()),
        input.css.source_map_source.as_deref().map(Into::into),
        input.css.content.as_str(),
    )?;
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &input.module_id,
        input.language,
        input.module_kind,
        &theme,
        &input.css.name,
        options,
    );
    binding_output(result, &source, &sources)
}

/// Formats typed errors and lint findings into the Node transport schema.
///
/// `source` must be the buffer used to produce `result`, and every published label must
/// resolve through `sources` to the exact original revision. Fatal diagnostics omit
/// transformed code and its mappings.
///
/// # Errors
///
/// Returns invalid reporting coordinates. Invalid source replacements become
/// compilation diagnostics with no generated output.
pub(super) fn binding_output(
    result: CompileResult,
    source: &SourceFile,
    sources: &SourceRegistry,
) -> Result<BindingOutput, BindingApiError> {
    Ok(binding_output_with_batch(result, source, sources)?.0)
}

/// Builds a transport result together with the resolved batch retained by a session.
pub(super) fn binding_output_with_batch(
    mut result: CompileResult,
    source: &SourceFile,
    sources: &SourceRegistry,
) -> Result<(BindingOutput, DiagnosticBatch), BindingApiError> {
    let (code, mappings) = if result.diagnostics.is_empty() {
        match render_transforms(source, &result.transforms) {
            Ok(output) => (Some(output.code), output.mappings),
            Err(error) => {
                let span = match &error {
                    TransformError::InvalidExpressionMapping(span)
                    | TransformError::Overlap { span, .. }
                        if source.slice(*span).is_ok() =>
                    {
                        *span
                    }
                    _ => source.full_span(),
                };
                result
                    .diagnostics
                    .push(CompileError::new(transform_failure_kind(&error), span));
                result.css.clear();
                result.classes.clear();
                result.symbols.clear();
                result.resources.clear();
                result.composition.clear();
                result.rejected_names.clear();
                (None, Vec::new())
            }
        }
    } else {
        (None, Vec::new())
    };
    let resolved = Reporter::collect(
        &result.diagnostics,
        &result.lint_findings,
        sources,
        result.lint_settings.clone(),
    )?;
    let diagnostics = resolved
        .diagnostics
        .iter()
        .take(result.diagnostics.len())
        .map(binding_diagnostic)
        .collect();
    let lint_findings = resolved
        .diagnostics
        .iter()
        .skip(result.diagnostics.len())
        .map(|diagnostic| BindingLintFinding {
            rule: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            uri: diagnostic.primary.location.uri.as_str().to_owned(),
            revision: diagnostic.primary.location.revision.clone(),
            start: diagnostic.primary.location.start,
            end: diagnostic.primary.location.end,
            start_line: diagnostic.primary.location.start_position.line,
            start_column: diagnostic.primary.location.start_position.utf16_column,
            end_line: diagnostic.primary.location.end_position.line,
            end_column: diagnostic.primary.location.end_position.utf16_column,
            notes: diagnostic.notes.clone(),
        })
        .collect();
    let reports = resolved
        .diagnostics
        .iter()
        .cloned()
        .map(BindingReport::from)
        .collect();
    Ok((
        BindingOutput {
            composition: result.composition.into_iter().map(Into::into).collect(),
            resources: result.resources.into_iter().map(Into::into).collect(),
            css: result.css,
            code,
            mappings: mappings.into_iter().map(Into::into).collect(),
            classes: result.classes.into_iter().map(Into::into).collect(),
            symbols: result.symbols.into_iter().map(Into::into).collect(),
            rejected_names: result.rejected_names.into_iter().map(Into::into).collect(),
            diagnostics,
            reports,
            lint_findings,
            lint_settings: result
                .lint_settings
                .iter()
                .map(|config| {
                    (
                        config.kind().name().to_owned(),
                        config.severity().as_str().to_owned(),
                    )
                })
                .collect(),
        },
        resolved,
    ))
}

fn binding_diagnostic(diagnostic: &ResolvedDiagnostic) -> BindingDiagnostic {
    let location = &diagnostic.primary.location;
    BindingDiagnostic {
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        uri: location.uri.as_str().to_owned(),
        revision: location.revision.clone(),
        start: location.start,
        end: location.end,
        start_utf16: location.start_utf16,
        end_utf16: location.end_utf16,
        start_line: location.start_position.line,
        start_column: location.start_position.utf16_column,
        end_line: location.end_position.line,
        end_column: location.end_position.utf16_column,
        notes: diagnostic.notes.clone(),
    }
}

fn transform_failure_kind(error: &TransformError) -> CompileDiagnostic {
    match error {
        TransformError::CompilationFailed => CompileDiagnostic::InvalidTransformState,
        TransformError::InvalidSpan(source) => {
            CompileDiagnostic::source_failure(source, SourceFailureContext::Transform)
        }
        TransformError::InvalidExpressionMapping(span) => {
            CompileDiagnostic::InvalidTransformExpressionMapping { span: *span }
        }
        TransformError::Overlap {
            span,
            previous_start,
        } => CompileDiagnostic::InvalidTransformOverlap {
            span: *span,
            previous_start: *previous_start,
        },
        TransformError::TooLarge => CompileDiagnostic::InvalidTransformSize,
    }
}

/// Compiles a complete source graph and returns one output per input module.
///
/// The source graph uses the same domain records as the typed Node adapter.
///
/// A direct input registers its editable document as the parser buffer. A projected
/// input registers separate generated and original buffers and reserves an analysis
/// file ID. Published spans refer to the editable document even when the generated
/// buffer contains the same text. Module identities are chosen by the host.
///
/// Composition findings are recomputed against all successful input artifacts before
/// outputs are built, so a base may appear after its consumer in `input`. Source
/// suppression runs in each consuming source; missing base records remain unknown.
///
/// Rules shared by two modules are emitted once: the first module in input order keeps
/// the rule and the later module's duplicate is dropped, because this output is one
/// document rather than one fragment per module.
///
/// # Errors
///
/// Returns an error for invalid options or sources, exhausted file IDs, or an
/// unresolved module edge, or a failure while resolving diagnostic sources or rendering
/// source transforms. Module compilation errors are carried in the returned outputs.
pub fn compile_project_input(
    input: &[HostPreparedSource],
    css: &CompileCssInput,
    options: &CompilerOptions,
) -> Result<ProjectBindingOutput, BindingApiError> {
    options.validate()?;
    let mut registry = SourceRegistry::default();
    let scans = input
        .iter()
        .map(|source| {
            let input = source
                .input(&mut registry)
                .map_err(|error| BindingApiError::Input(error.reason))?;
            Ok::<_, BindingApiError>(OxcSourceAnalyzer.scan_input(input))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let theme = registry.register(
        Some(css.uri.clone()),
        css.source_map_source.as_deref().map(Into::into),
        css.content.as_str(),
    )?;
    let assets = scans
        .iter()
        .flat_map(|scan| scan.summary.assets.iter().cloned())
        .map(|asset| (asset.identity, asset.metadata))
        .collect::<BTreeMap<IdentityId, AssetMetadata>>();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, css.name.clone(), options.design_system())
            .map_err(BindingApiError::DesignSystem)?;
    let compiler = ModuleCompiler::new(snapshot.clone(), options.clone());
    let mut results = scans
        .iter()
        .zip(input)
        .map(|(scan, source)| {
            let type_facts = source.type_facts();
            let facts = TypeFacts {
                schema: TypeFacts::SCHEMA,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of(format!("{type_facts:?}{assets:?}")),
                facts: type_facts.to_vec(),
                assets: assets.clone(),
            };
            compiler.compile(
                &OxcSourceAnalyzer.finalize(scan.clone(), Some(&facts)),
                &Identifiers::default(),
            )
        })
        .collect::<Vec<_>>();
    let mut resolver =
        StyleResolver::new(results.iter().flat_map(|artifact| &artifact.style_effects));
    let findings = results
        .iter()
        .zip(&scans)
        .map(|(artifact, scan)| {
            let mut findings = artifact
                .style_effects
                .iter()
                .filter_map(|effects| resolver.resolve(effects.identity()))
                .flat_map(|style| check_style(&style))
                .collect::<Vec<_>>();
            filter_lint_findings(&scan.source, &mut findings);
            findings
        })
        .collect::<Vec<_>>();
    for (artifact, findings) in results.iter_mut().zip(findings) {
        artifact
            .lint_findings
            .retain(|finding| !is_style_analysis_lint(finding.kind()));
        artifact.lint_findings.extend(findings);
    }
    let mut emitted_css_rules = BTreeSet::new();
    for result in &mut results {
        result
            .css
            .rules
            .retain(|rule| emitted_css_rules.insert(rule.key.clone()));
    }
    let modules = scans
        .iter()
        .zip(results)
        .map(|(scan, result)| {
            let result = result.to_module_output(&snapshot);
            Ok(ProjectModuleOutput::new(
                scan.module_id.clone(),
                binding_output(result, &scan.source, &registry)?,
            ))
        })
        .collect::<Result<Vec<_>, BindingApiError>>()?;
    Ok(ProjectBindingOutput { modules })
}

/// Scans plain or projected input and returns original UTF-8 summary coordinates.
///
/// # Errors
///
/// Rejects malformed input and projection failures before checker queries are issued.
#[napi(strict)]
pub fn scan_source(source: NativePreparedSource) -> napi::Result<NativeModuleSummary> {
    let source = HostPreparedSource::try_from(source)?;
    let mut registry = SourceRegistry::default();
    let scan = OxcSourceAnalyzer.scan_input(source.input(&mut registry)?);
    if let Some(error) = scan.diagnostics.iter().find(|error| {
        error.diagnostic.is_projection_failure()
            || matches!(error.diagnostic, CompileDiagnostic::JavaScriptParse)
    }) {
        return Err(Error::from_reason(error.to_string()));
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
    let wanted: BTreeSet<_> = result
        .type_probes
        .iter()
        .map(|probe| (probe.start, probe.end))
        .chain(
            result
                .assets
                .iter()
                .map(|asset| (asset.call_start, asset.call_end)),
        )
        .chain(
            result
                .module_specifiers
                .iter()
                .map(|specifier| (specifier.start, specifier.end)),
        )
        .collect();
    let index = CoordinateIndex::new(scan.input.parser_source());
    for mapped in scan.provenance {
        if !wanted.contains(&(mapped.original.start.0, mapped.original.end.0)) {
            continue;
        }
        result.checker_spans.push(NativeCheckerSpan {
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
    Ok(result)
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

/// Host metadata and CSS text for a compilation.
#[napi(object, object_to_js = false)]
pub struct NativeCompileCssInput {
    /// CSS source text.
    pub content: String,
    /// Opaque identity used for CSS import and resource resolution.
    pub name: String,
    /// Absolute URI of the editable CSS document.
    pub uri: String,
    /// Exact source name for generated CSS maps.
    pub source_map_source: Option<String>,
}

impl TryFrom<NativeCompileCssInput> for CompileCssInput {
    type Error = Error;

    fn try_from(input: NativeCompileCssInput) -> Result<Self, Self::Error> {
        Ok(Self {
            content: input.content,
            name: input.name.into(),
            uri: SourceUri::parse(input.uri)
                .map_err(|error| Error::from_reason(error.to_string()))?,
            source_map_source: input.source_map_source,
        })
    }
}

/// Host metadata and source texts for a standalone compilation.
#[napi(object, object_to_js = false)]
pub struct NativeCompileSourceInput {
    /// Parser input text.
    pub source: String,
    /// Module identity used for generated names and semantic references.
    pub module_id: String,
    /// Absolute URI of the editable script document.
    pub uri: String,
    /// Exact source name for generated CSS maps.
    pub source_map_source: Option<String>,
    /// Parser dialect chosen by the host.
    pub language: NativeScriptLanguage,
    /// Module grammar chosen by the host.
    pub module_kind: NativeScriptModuleKind,
    /// CSS source and its identities.
    pub css: NativeCompileCssInput,
}

/// Compiles one host-identified script and CSS source.
///
/// # Errors
///
/// Returns a JavaScript error for invalid URIs, sources, or options.
#[napi(strict)]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn compile_source(
    input: NativeCompileSourceInput,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<BindingOutput> {
    let options = compiler_options(options)?;
    let input = CompileSourceInput {
        source: input.source,
        module_id: input.module_id.into(),
        uri: SourceUri::parse(input.uri).map_err(|error| Error::from_reason(error.to_string()))?,
        source_map_source: input.source_map_source,
        language: input.language.into(),
        module_kind: input.module_kind.into(),
        css: input.css.try_into()?,
    };
    compile_source_input(&input, &options).map_err(|error| Error::from_reason(error.to_string()))
}

/// Compiles a resolved module graph in one native invocation.
///
/// # Errors
///
/// Returns a JavaScript error for invalid input or an unresolved graph edge.
#[napi(strict)]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn compile_project(
    sources: Vec<NativePreparedSource>,
    css: NativeCompileCssInput,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<ProjectBindingOutput> {
    let options = compiler_options(options)?;
    let sources = sources
        .into_iter()
        .map(TryInto::try_into)
        .collect::<napi::Result<Vec<HostPreparedSource>>>()?;
    compile_project_input(&sources, &css.try_into()?, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Wraps project module CSS fragments in one shared theme/base stylesheet.
///
/// # Errors
///
/// Returns a JavaScript error when the theme or compiler options are invalid.
#[napi(strict)]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn render_stylesheet(
    module_css: String,
    theme: String,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<String> {
    let options = compiler_options(options)?;
    let theme = SourceFile::new(FileId(1), None, None, theme).map_err(BindingApiError::from)?;
    Compiler::render_stylesheet(
        &theme,
        &CssSourceName::from("stillwind.css"),
        &module_css,
        &options,
    )
    .map_err(|error| Error::from_reason(error.to_string()))
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

/// Generates the TypeScript vocabulary augmentation for one CSS-first theme.
///
/// # Errors
///
/// Returns a JavaScript error when the theme is invalid or vocabulary generation fails.
#[napi]
pub fn generate_vocabulary_types(theme: String) -> napi::Result<String> {
    let source = SourceFile::new(FileId(2), None, None, theme).map_err(BindingApiError::from)?;
    Compiler::generate_vocabulary_types(&source, &CssSourceName::from("stillwind.css"))
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Canonicalizes constant `calc()` subexpressions with Stillwind's CSS value parser.
/// Exposed so parity and integration tools compare both compilers after the same
/// semantic normalization. Expressions that do not parse, or that nest values deeper
/// than the calculator accepts, are retained byte-for-byte.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn fold_calc_expressions(css: String, zero_unit: Option<String>) -> String {
    stillwind_css::fold_calc_expressions(&css, zero_unit.as_deref())
}

#[cfg(test)]
mod tests {
    use stillwind_compiler::SourceTransform;
    use stillwind_source::Span;

    use super::*;

    #[test]
    fn binding_output_preserves_transform_and_diagnostic_span_failures() {
        let mut registry = SourceRegistry::default();
        let source = registry
            .register(
                Some(SourceUri::parse("untitled:entry").unwrap()),
                None,
                "abc",
            )
            .unwrap();
        let mut result = CompileResult::default();
        result.transforms.push(SourceTransform {
            call_span: Span::new(FileId(3), 0, 1),
            replacement: "x".into(),
            expressions: Vec::new(),
        });
        let output = binding_output(result, &source, &registry).unwrap();
        assert!(output.code.is_none());
        assert!(output.css.is_empty());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "SW09004");
        assert!(output.diagnostics[0].message.contains("expected FileId("));

        let mut result = CompileResult::default();
        result.diagnostics.push(CompileError::new(
            CompileDiagnostic::InvalidStyleInput,
            Span::new(FileId(3), 0, 1),
        ));
        assert!(matches!(
            binding_output(result, &source, &registry),
            Err(BindingApiError::Aggregate(AggregateError::Source(
                SourceRegistryError::UnknownFile(_)
            )))
        ));
    }

    #[test]
    fn naming_revision_reaches_the_artifact_key() {
        let source = SourceFile::new(FileId(1), None, None, "ref").unwrap();
        let theme = SourceFile::new(FileId(2), None, None, "@theme {}").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            "test.css".into(),
            CompilerOptions::default().design_system(),
        )
        .unwrap();
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                std::sync::Arc::new(source),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
        let key = |naming_revision: &str| {
            ModuleCompiler::new(
                snapshot.clone(),
                CompilerOptions {
                    naming_revision: naming_revision.into(),
                    ..CompilerOptions::default()
                },
            )
            .key_for(&analysis)
        };
        assert_ne!(
            key("builtin").compiler_options,
            key("custom-1").compiler_options
        );
        assert_eq!(
            key("custom-1").compiler_options,
            key("custom-1").compiler_options
        );
    }

    #[test]
    fn object_options_apply_defaults_and_validate_values() {
        assert_eq!(compiler_options(None).unwrap(), CompilerOptions::default());
        assert_eq!(
            compiler_options(Some(NativeCompilerOptions::default())).unwrap(),
            CompilerOptions::default()
        );
        assert_eq!(
            compiler_options(Some(NativeCompilerOptions {
                naming_revision: Some("host-hash".into()),
                ..Default::default()
            }))
            .unwrap()
            .naming_revision,
            "host-hash"
        );
        assert!(
            compiler_options(Some(NativeCompilerOptions {
                prefix: Some("bad".into()),
                ..Default::default()
            }))
            .is_err()
        );
        assert_eq!(
            compiler_options(Some(NativeCompilerOptions {
                hash_length: Some(12),
                ..Default::default()
            }))
            .unwrap()
            .hash_length,
            12
        );
        for rejected in [0, 51] {
            assert!(
                compiler_options(Some(NativeCompilerOptions {
                    hash_length: Some(rejected),
                    ..Default::default()
                }))
                .is_err(),
                "{rejected}"
            );
        }
    }
}
