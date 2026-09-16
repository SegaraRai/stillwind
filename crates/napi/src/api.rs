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
    CompileResult, Compiler, CompilerOptions, ModuleCompiler, render_transforms,
};
use stillwind_design_system::{DesignSystemError, DesignSystemSnapshot};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFacts};
use stillwind_source::{FileId, Fingerprint, SourceError, SourceFile};

use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{BindingDiagnostic, BindingLintFinding, BindingOutput};
use crate::output::{ProjectBindingOutput, ProjectModuleOutput};
use crate::semantic::{NativeModuleSummary, NativeTypeProbe};
use crate::source_input::{NativeProjectSource, ProjectSource};

/// Failures that can occur before any compiler diagnostic is produced.
#[derive(Debug, thiserror::Error)]
pub enum BindingApiError {
    /// A host value was structurally invalid, such as a projection without its required
    /// companion field.
    #[error("invalid source input: {0}")]
    Input(String),
    /// A source buffer or coordinate was rejected.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// The source graph needs more file ids than `u32` allows.
    #[error("source graph exceeds the 32-bit file ID limit")]
    TooManySources,
    /// The design system CSS failed to load or link.
    #[error("{0}")]
    DesignSystem(#[from] DesignSystemError),
    /// Compiler options were rejected.
    #[error("{0}")]
    Options(#[from] CompileErrorKind),
}

impl From<BindingApiError> for Error {
    fn from(error: BindingApiError) -> Self {
        Self::from_reason(error.to_string())
    }
}

/// Compiles a source buffer and returns the serializable compiler result.
///
/// Kept independent of Node-API so the same contract has an ordinary Rust E2E test as
/// well as a JavaScript consumer.
///
/// # Errors
///
/// Returns invalid source coordinates or compiler options.
pub fn compile_react_input(
    source: &str,
    source_path: &str,
    theme: &str,
) -> Result<BindingOutput, BindingApiError> {
    compile_react_input_with_options(source, source_path, theme, &CompilerOptions::default())
}

/// Compiles one module with explicitly chosen compiler options.
pub fn compile_react_input_with_options(
    source: &str,
    source_path: &str,
    theme: &str,
    options: &CompilerOptions,
) -> Result<BindingOutput, BindingApiError> {
    options.validate()?;
    let source = SourceFile::new(FileId(1), source_path, source)?;
    let theme =
        SourceFile::new(FileId(2), "stillwind.css", theme).map_err(BindingApiError::from)?;
    let result =
        Compiler::compile_source_with_options(&OxcSourceAnalyzer, &source, &theme, options);
    Ok(binding_output(result, &source, &theme))
}

/// Formats typed errors and lint findings into the Node transport schema.
///
/// `source` and `theme` must be the buffers used to produce `result`. Primary labels
/// select the reporting buffer; codes, messages, and notes are serialized separately.
/// Fatal diagnostics or failed source edits omit transformed code and its mappings.
pub(super) fn binding_output(
    result: CompileResult,
    source: &SourceFile,
    theme: &SourceFile,
) -> BindingOutput {
    let (code, mappings) = if result.diagnostics.is_empty() {
        match render_transforms(source, &result.transforms) {
            Ok(output) => (Some(output.code), output.mappings),
            Err(_) => (None, Vec::new()),
        }
    } else {
        (None, Vec::new())
    };
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
                code: diagnostic.code().to_owned(),
                message: diagnostic.to_string(),
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
                rule: finding.code().to_owned(),
                message: finding.to_string(),
                file: finding_source.path().to_owned(),
                start: span.start.0,
                end: span.end.0,
                notes: finding.notes.clone(),
            }
        })
        .collect();
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
        lint_findings,
        lint_policy: result.lint_policy,
    }
}

/// Compiles a complete source graph and returns one output per input module.
///
/// The source graph uses the same domain records as the typed Node adapter.
///
/// Every input receives three file ids: the original source, the generated buffer, and
/// the analysis buffer. The original of module `i` and the code of a projected module
/// are different files even when their text matches, and only the original may appear
/// in a published local span. Original file ids occupy the first range; generated
/// and analysis ids follow it. Module identities themselves are host path strings.
///
/// Rules shared by two modules are emitted once: the first module in input order keeps
/// the rule and the later module's duplicate is dropped, because this output is one
/// document rather than one fragment per module.
///
/// # Errors
///
/// Returns invalid sources, options, or unresolved module edges.
pub fn compile_react_project_input(
    input: &[ProjectSource],
    theme: &str,
) -> Result<ProjectBindingOutput, BindingApiError> {
    compile_react_project_input_with_options(input, theme, &CompilerOptions::default())
}

/// Compiles a whole source graph with explicitly chosen compiler options.
pub fn compile_react_project_input_with_options(
    input: &[ProjectSource],
    theme: &str,
    options: &CompilerOptions,
) -> Result<ProjectBindingOutput, BindingApiError> {
    options.validate()?;
    let scans = input
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let generated_index = input
                .len()
                .checked_add(
                    index
                        .checked_mul(2)
                        .ok_or(BindingApiError::TooManySources)?,
                )
                .and_then(|index| index.checked_add(1))
                .ok_or(BindingApiError::TooManySources)?;
            let analysis_index = generated_index
                .checked_add(1)
                .ok_or(BindingApiError::TooManySources)?;
            let input = source
                .input(
                    source_file_id(index)?,
                    source_file_id(generated_index)?,
                    source_file_id(analysis_index)?,
                )
                .map_err(|error| BindingApiError::Input(error.reason))?;
            Ok::<_, BindingApiError>(OxcSourceAnalyzer.scan_input(input))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let theme = SourceFile::new(source_file_id(scans.len())?, "stillwind.css", theme)?;
    let assets = scans
        .iter()
        .flat_map(|scan| scan.summary.assets.iter().cloned())
        .map(|asset| (asset.identity, asset.metadata))
        .collect::<BTreeMap<IdentityId, AssetMetadata>>();
    let snapshot = DesignSystemSnapshot::from_source(&theme, options.design_system())
        .map_err(BindingApiError::DesignSystem)?;
    let compiler = ModuleCompiler::new(snapshot.clone(), options.clone());
    let mut results = scans
        .iter()
        .zip(input)
        .map(|(scan, source)| {
            let type_facts = &source.type_facts;
            let facts = TypeFacts {
                schema: TypeFacts::SCHEMA,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of(format!("{type_facts:?}{assets:?}")),
                facts: type_facts.clone(),
                assets: assets.clone(),
            };
            compiler.compile(&OxcSourceAnalyzer.finalize(scan.clone(), Some(&facts)))
        })
        .collect::<Vec<_>>();
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
            ProjectModuleOutput::new(
                scan.source.path().to_owned(),
                binding_output(result, &scan.source, &theme),
            )
        })
        .collect();
    Ok(ProjectBindingOutput { modules })
}

/// Extracts the versioned cross-module summary for a React source module.
///
/// # Errors
///
/// Returns invalid source coordinates.
pub fn summarize_react_input(
    source: &str,
    source_path: &str,
) -> Result<NativeModuleSummary, BindingApiError> {
    let source = SourceFile::new(FileId(1), source_path, source)?;
    let parsed = parse(&source);
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
                start: span.start.0,
                end: span.end.0,
            })
            .collect(),
    ))
}

/// Native Node entrypoint used by `@stillwind/vite`.
///
/// napi-rs converts inputs and outputs directly to JavaScript values.
///
/// # Errors
///
/// Returns a JavaScript error for invalid source or options.
#[napi(js_name = "compileReact", strict)]
pub fn compile_react(
    source: String,
    source_path: Option<String>,
    theme: String,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<BindingOutput> {
    let options = compiler_options(options)?;
    compile_react_input_with_options(
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
#[napi(js_name = "compileReactProject", strict)]
pub fn compile_react_project(
    sources: Vec<NativeProjectSource>,
    theme: String,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<ProjectBindingOutput> {
    let options = compiler_options(options)?;
    let sources = sources
        .into_iter()
        .map(TryInto::try_into)
        .collect::<napi::Result<Vec<ProjectSource>>>()?;
    compile_react_project_input_with_options(&sources, &theme, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Wraps project module CSS fragments in one shared theme/base stylesheet.
///
/// # Errors
///
/// Returns a JavaScript error when the theme or compiler options are invalid.
#[napi(js_name = "renderReactStylesheet", strict)]
pub fn render_react_stylesheet(
    module_css: String,
    theme: String,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<String> {
    let options = compiler_options(options)?;
    let theme =
        SourceFile::new(FileId(1), "stillwind.css", theme).map_err(BindingApiError::from)?;
    Compiler::render_react_stylesheet(&theme, &module_css, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Returns the versioned Stillwind module summary used for cross-module values.
///
/// # Errors
///
/// Returns a JavaScript error for invalid source coordinates.
#[napi(js_name = "summarizeReact")]
pub fn summarize_react(
    source: String,
    source_path: Option<String>,
) -> napi::Result<NativeModuleSummary> {
    summarize_react_input(&source, source_path.as_deref().unwrap_or("Component.tsx"))
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Generates the TypeScript vocabulary augmentation for one CSS-first theme.
#[napi(js_name = "generateReactTypes")]
pub fn generate_react_types(theme: String) -> napi::Result<String> {
    let source =
        SourceFile::new(FileId(2), "stillwind.css", theme).map_err(BindingApiError::from)?;
    Compiler::generate_react_types(&source).map_err(|error| Error::from_reason(error.to_string()))
}

/// Canonicalizes constant `calc()` subexpressions with Stillwind's CSS value parser.
/// Exposed so parity and integration tools compare both compilers after the same
/// semantic normalization. Expressions that do not parse, or that nest values deeper
/// than the calculator accepts, are retained byte-for-byte.
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
    fn naming_revision_reaches_the_artifact_key() {
        let source = SourceFile::new(FileId(1), "src/Menu.tsx", "ref").unwrap();
        let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}").unwrap();
        let snapshot =
            DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
                .unwrap();
        let analysis =
            OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(source)), None);
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
