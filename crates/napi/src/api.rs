use std::collections::{BTreeMap, BTreeSet};

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions, ModuleCompiler};
use stillwind_design_system::{DesignSystemError, DesignSystemOptionsError, DesignSystemSnapshot};
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFacts};
use stillwind_source::{FileId, Fingerprint, SourceFile};

use super::output::{BindingDiagnostic, BindingLintFinding, BindingMapping, BindingOutput};
use super::session::{ProjectBindingOutput, ProjectModuleOutput, ProjectSource};

#[derive(Debug, thiserror::Error)]
pub enum BindingApiError {
    #[error("{0}")]
    Source(#[from] stillwind_source::SourceError),
    #[error("source graph exceeds the 32-bit file ID limit")]
    TooManySources,
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    DesignSystem(#[from] DesignSystemError),
    #[error("{0}")]
    Options(#[from] DesignSystemOptionsError),
}

impl From<BindingApiError> for Error {
    fn from(error: BindingApiError) -> Self {
        Self::from_reason(error.to_string())
    }
}

/// Compiles a source buffer and returns the serializable compiler result.
///
/// Kept independent of Node-API so the same contract has an ordinary Rust E2E
/// test as well as a JavaScript consumer.
///
/// # Errors
///
/// Returns an error only when serializing a successful compiler result fails.
pub fn compile_react_json(
    source: &str,
    source_path: &str,
    theme: &str,
) -> Result<String, BindingApiError> {
    compile_react_json_with_options(source, source_path, theme, &CompilerOptions::default())
}

pub fn compile_react_json_with_options(
    source: &str,
    source_path: &str,
    theme: &str,
    options: &CompilerOptions,
) -> Result<String, BindingApiError> {
    let source = SourceFile::new(FileId(1), source_path, source)?;
    let theme =
        SourceFile::new(FileId(2), "stillwind.css", theme).map_err(BindingApiError::from)?;
    let result =
        Compiler::compile_source_with_options(&OxcSourceAnalyzer, &source, &theme, options);
    serde_json::to_string(&binding_output(result, &source, &theme)).map_err(BindingApiError::Json)
}

pub(super) fn binding_output(
    result: CompileResult,
    source: &SourceFile,
    theme: &SourceFile,
) -> BindingOutput {
    let code = result.apply_to_source(source).ok();
    let mut transforms = result.transforms.iter().collect::<Vec<_>>();
    transforms.sort_by_key(|transform| transform.call_span.start.0);
    let mut generated_delta = 0_i64;
    let mappings = transforms
        .into_iter()
        .filter_map(|transform| {
            let original_start = source.utf16_offset(transform.call_span.start).ok()?;
            let original_end = source.utf16_offset(transform.call_span.end).ok()?;
            let generated_start = i64::from(original_start) + generated_delta;
            let replacement_length =
                u32::try_from(transform.replacement.encode_utf16().count()).ok()?;
            let generated_end = generated_start + i64::from(replacement_length);
            generated_delta +=
                i64::from(replacement_length) - i64::from(original_end - original_start);
            Some(BindingMapping {
                original_start,
                original_end,
                generated_start: u32::try_from(generated_start).ok()?,
                generated_end: u32::try_from(generated_end).ok()?,
            })
        })
        .collect();
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            let diagnostic_source = if span.file == theme.id() {
                &theme
            } else {
                &source
            };
            let start_position = diagnostic_source.position(span.start).ok();
            let end_position = diagnostic_source.position(span.end).ok();
            BindingDiagnostic {
                code: diagnostic.code.to_owned(),
                message: diagnostic.message.clone(),
                file: diagnostic_source.path().to_owned(),
                start: span.start.0,
                end: span.end.0,
                start_utf16: diagnostic_source
                    .utf16_offset(span.start)
                    .unwrap_or_default(),
                end_utf16: diagnostic_source.utf16_offset(span.end).unwrap_or_default(),
                start_line: start_position.map_or(0, |position| position.line),
                start_column: start_position.map_or(0, |position| position.utf16_column),
                end_line: end_position.map_or(0, |position| position.line),
                end_column: end_position.map_or(0, |position| position.utf16_column),
                notes: diagnostic.notes.clone(),
            }
        })
        .collect();
    let lint_findings = result
        .lint_findings
        .iter()
        .map(|finding| {
            let span = finding
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            let finding_source = if span.file == theme.id() {
                theme
            } else {
                source
            };
            BindingLintFinding {
                rule: finding.rule.to_owned(),
                message: finding.message.clone(),
                file: finding_source.path().to_owned(),
                start: span.start.0,
                end: span.end.0,
                notes: finding.notes.clone(),
            }
        })
        .collect();
    BindingOutput {
        resources: result.resources,
        css: result.css,
        code,
        mappings,
        classes: result.classes,
        symbols: result.symbols,
        diagnostics,
        lint_findings,
        lint_policy: result.lint_policy,
    }
}

/// Compiles a complete source graph and returns one output per input module.
///
/// The source graph is encoded as a JSON array of `{ path, source }` records so
/// the native ABI can remain stable while bundler adapters evolve.
///
/// # Errors
///
/// Returns an error when the input JSON is invalid, module edges cannot be
/// resolved, or the compiler output cannot be serialized.
pub fn compile_react_project_json(
    sources_json: &str,
    theme: &str,
) -> Result<String, BindingApiError> {
    compile_react_project_json_with_options(sources_json, theme, &CompilerOptions::default())
}

pub fn compile_react_project_json_with_options(
    sources_json: &str,
    theme: &str,
    options: &CompilerOptions,
) -> Result<String, BindingApiError> {
    let input: Vec<ProjectSource> = serde_json::from_str(sources_json)?;
    let sources = input
        .into_iter()
        .enumerate()
        .map(|(index, source)| {
            Ok::<_, BindingApiError>((
                SourceFile::new(source_file_id(index)?, source.path, source.source)?,
                source.type_facts,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let theme = SourceFile::new(source_file_id(sources.len())?, "stillwind.css", theme)?;
    let assets = sources
        .iter()
        .flat_map(|(source, _)| parse(source).summary.assets)
        .map(|asset| (asset.identity, asset.metadata))
        .collect::<BTreeMap<IdentityId, AssetMetadata>>();
    let snapshot = DesignSystemSnapshot::from_source(&theme, options.design_system())
        .map_err(BindingApiError::DesignSystem)?;
    let compiler = ModuleCompiler::new(snapshot.clone(), options.clone());
    let mut results = sources
        .iter()
        .map(|(source, type_facts)| {
            let scan = OxcSourceAnalyzer.scan(std::sync::Arc::new(source.clone()));
            let facts = TypeFacts {
                schema: TypeFacts::SCHEMA,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of(format!("{type_facts:?}{assets:?}")),
                facts: type_facts.clone(),
                assets: assets.clone(),
            };
            compiler.compile(&OxcSourceAnalyzer.finalize(scan, Some(&facts)))
        })
        .collect::<Vec<_>>();
    let mut emitted_css_rules = BTreeSet::new();
    for result in &mut results {
        result
            .css
            .rules
            .retain(|rule| emitted_css_rules.insert(rule.key.clone()));
    }
    let modules = sources
        .iter()
        .zip(results)
        .map(|((source, _), result)| {
            let mut result = result.to_output(&snapshot, options);
            // Project consumers assemble one shared stylesheet from these
            // fragments instead of repeating theme/base layers per module.
            result.css.clone_from(&result.module_css);
            ProjectModuleOutput {
                path: source.path().to_owned(),
                output: binding_output(result, source, &theme),
            }
        })
        .collect();
    serde_json::to_string(&ProjectBindingOutput { modules }).map_err(BindingApiError::Json)
}

/// Extracts the versioned cross-module summary for a React source module.
///
/// # Errors
///
/// Returns an error only when serializing the summary fails.
pub fn summarize_react_json(source: &str, source_path: &str) -> Result<String, BindingApiError> {
    let source = SourceFile::new(FileId(1), source_path, source)?;
    let parsed = parse(&source);
    let mut summary = serde_json::to_value(&parsed.summary).map_err(BindingApiError::Json)?;
    summary["type_probes"] = serde_json::json!(
        parsed
            .type_probes
            .iter()
            .map(|span| { serde_json::json!({ "start": span.start.0, "end": span.end.0 }) })
            .collect::<Vec<_>>()
    );
    serde_json::to_string(&summary).map_err(BindingApiError::Json)
}

/// Native Node entrypoint used by `@stillwind/vite`.
///
/// The result is JSON to keep the N-API surface stable and exactly aligned with
/// the CLI protocol during the migration period.
///
/// # Errors
///
/// Returns a JavaScript error if the compiler result cannot be serialized.
#[napi(js_name = "compileReact")]
pub fn compile_react(
    source: String,
    source_path: Option<String>,
    theme: String,
    options_json: Option<String>,
) -> napi::Result<String> {
    let options = parse_options(options_json.as_deref())?;
    compile_react_json_with_options(
        &source,
        source_path.as_deref().unwrap_or("Component.tsx"),
        &theme,
        &options,
    )
    .map_err(|error| Error::from_reason(error.to_string()))
}

/// Compiles a resolved React module graph in one native invocation.
///
/// # Errors
///
/// Returns a JavaScript error for invalid input or an unresolved graph edge.
#[napi(js_name = "compileReactProject")]
pub fn compile_react_project(
    sources_json: String,
    theme: String,
    options_json: Option<String>,
) -> napi::Result<String> {
    let options = parse_options(options_json.as_deref())?;
    compile_react_project_json_with_options(&sources_json, &theme, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Wraps project module CSS fragments in one shared theme/base stylesheet.
///
/// # Errors
///
/// Returns a JavaScript error when the theme or compiler options are invalid.
#[napi(js_name = "renderReactStylesheet")]
pub fn render_react_stylesheet(
    module_css: String,
    theme: String,
    options_json: Option<String>,
) -> napi::Result<String> {
    let options = parse_options(options_json.as_deref())?;
    let theme =
        SourceFile::new(FileId(1), "stillwind.css", theme).map_err(BindingApiError::from)?;
    Compiler::render_react_stylesheet(&theme, &module_css, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

pub(super) fn parse_options(
    options_json: Option<&str>,
) -> Result<CompilerOptions, BindingApiError> {
    let options = options_json.map_or_else(
        || Ok(CompilerOptions::default()),
        |value| serde_json::from_str(value).map_err(BindingApiError::Json),
    )?;
    options.validate()?;
    Ok(options)
}

/// Returns the versioned Stillwind module summary used for cross-module values.
///
/// # Errors
///
/// Returns a JavaScript error if the summary cannot be serialized.
#[napi(js_name = "summarizeReact")]
pub fn summarize_react(source: String, source_path: Option<String>) -> napi::Result<String> {
    summarize_react_json(&source, source_path.as_deref().unwrap_or("Component.tsx"))
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Generates the TypeScript vocabulary augmentation for one CSS-first theme.
#[napi(js_name = "generateReactTypes")]
pub fn generate_react_types(theme: String) -> napi::Result<String> {
    let source =
        SourceFile::new(FileId(2), "stillwind.css", theme).map_err(BindingApiError::from)?;
    Compiler::generate_react_types(&source).map_err(|error| Error::from_reason(error.to_string()))
}

/// Canonicalizes constant `calc()` subexpressions with Stillwind's CSS value
/// parser. Exposed so parity and integration tools compare both compilers
/// after the same semantic normalization.
#[napi(js_name = "foldCalcExpressions")]
pub fn fold_calc_expressions(css: String) -> String {
    stillwind_css::fold_calc_expressions(&css)
}

fn source_file_id(index: usize) -> Result<FileId, BindingApiError> {
    let id = index
        .checked_add(1)
        .ok_or(BindingApiError::TooManySources)?;
    u32::try_from(id)
        .map(FileId)
        .map_err(|_| BindingApiError::TooManySources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_options_apply_defaults_and_keep_parse_and_validation_failures_distinct() {
        assert_eq!(parse_options(None).unwrap(), CompilerOptions::default());
        assert_eq!(
            parse_options(Some("{}")).unwrap(),
            CompilerOptions::default()
        );
        assert!(matches!(
            parse_options(Some("{")),
            Err(BindingApiError::Json(_))
        ));
        assert!(parse_options(Some(r#"{"prefix":"bad"}"#)).is_err());
    }

    #[test]
    fn file_id_exhaustion_does_not_require_allocating_a_source_graph() {
        assert_eq!(source_file_id(0).unwrap(), FileId(1));
        assert_eq!(
            source_file_id(u32::MAX as usize - 1).unwrap(),
            FileId(u32::MAX)
        );
        assert!(matches!(
            source_file_id(u32::MAX as usize),
            Err(BindingApiError::TooManySources)
        ));
        assert!(matches!(
            source_file_id(usize::MAX),
            Err(BindingApiError::TooManySources)
        ));
    }
}
