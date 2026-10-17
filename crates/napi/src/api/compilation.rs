//! Compiles one source and converts compiler results into host output.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{
    CompileResult, Compiler, CompilerOptions, TransformError, render_transforms,
};
use stillwind_design_system::DesignSystemError;
use stillwind_diagnostics::{
    AggregateError, CompileDiagnostic, CompileError, DiagnosticBatch, LabelScope, LintKind,
    Reporter, ResolvedDiagnostic, SourceFailureContext,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{
    CssSourceName, ModuleId, SourceError, SourceFile, SourceRegistry, SourceRegistryError,
    SourceUri,
};

use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{BindingDiagnostic, BindingLintFinding, BindingOutput, BindingReport};
use crate::source_input::{NativeScriptLanguage, NativeScriptModuleKind};

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
        &OxcSourceAnalyzer::with_lint(options.lint_settings().has_enabled_rules()),
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
pub(crate) fn binding_output_with_batch(
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
            rule: (diagnostic.code.clone()).into(),
            message: (diagnostic.message.clone()).into(),
            uri: diagnostic.primary.location.uri.clone().into(),
            revision: (diagnostic.primary.location.revision.clone()).into(),
            start: diagnostic.primary.location.start,
            end: diagnostic.primary.location.end,
            start_line: diagnostic.primary.location.start_position.line,
            start_column: diagnostic.primary.location.start_position.utf16_column,
            end_line: diagnostic.primary.location.end_position.line,
            end_column: diagnostic.primary.location.end_position.utf16_column,
            notes: diagnostic.notes.iter().cloned().map(Into::into).collect(),
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
            css: (result.css).into(),
            code: (code).map(Into::into),
            mappings: mappings.into_iter().map(Into::into).collect(),
            classes: result.classes.into_iter().map(Into::into).collect(),
            symbols: result.symbols.into_iter().map(Into::into).collect(),
            rejected_names: result.rejected_names.into_iter().map(Into::into).collect(),
            dynamic_candidates: result
                .dynamic_candidates
                .iter()
                .map(ToString::to_string)
                .collect(),
            diagnostics,
            reports,
            lint_findings,
            lint_settings: LintKind::ALL
                .iter()
                .map(|&kind| {
                    (
                        kind.name().to_owned(),
                        result
                            .lint_settings
                            .reporting_severity(kind)
                            .as_str()
                            .to_owned(),
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
        scope: match diagnostic.primary.scope {
            LabelScope::Range => "range",
            LabelScope::Document => "document",
        }
        .into(),
        code: (diagnostic.code.clone()).into(),
        class: diagnostic.class.map(|class| class.as_str().into()),
        message: (diagnostic.message.clone()).into(),
        uri: location.uri.clone().into(),
        revision: (location.revision.clone()).into(),
        start: location.start,
        end: location.end,
        start_utf16: location.start_utf16,
        end_utf16: location.end_utf16,
        start_line: location.start_position.line,
        start_column: location.start_position.utf16_column,
        end_line: location.end_position.line,
        end_column: location.end_position.utf16_column,
        notes: diagnostic.notes.iter().cloned().map(Into::into).collect(),
    }
}

fn transform_failure_kind(error: &TransformError) -> CompileDiagnostic {
    match error {
        TransformError::CompilationFailed => CompileDiagnostic::TransformAfterFailedCompilation,
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
        TransformError::TooLarge => CompileDiagnostic::RenderedSourceTooLarge,
    }
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

#[cfg(test)]
mod tests {
    use stillwind_compiler::{ModuleCompiler, SourceTransform};
    use stillwind_design_system::DesignSystemSnapshot;
    use stillwind_ir::SourceAnalyzer;
    use stillwind_source::{FileId, Span};

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
        assert_eq!(output.css, "");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "SW33004");
        assert!(output.diagnostics[0].message.contains("expected FileId("));

        let mut result = CompileResult::default();
        result.diagnostics.push(CompileError::new(
            CompileDiagnostic::FrameworkConstraintConflict,
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
            key("builtin").compiler_options(),
            key("custom-1").compiler_options()
        );
        assert_eq!(
            key("custom-1").compiler_options(),
            key("custom-1").compiler_options()
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
