//! Node-API surface for running the Shorth compiler inside Vite.

use napi::{Error, Result};
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use shorth_core::{CompileResult, Compiler, GeneratedClass, GeneratedSymbol};
use shorth_source::{FileId, SourceFile};

#[derive(Serialize)]
struct BindingOutput {
    css: String,
    code: Option<String>,
    classes: Vec<GeneratedClass>,
    symbols: Vec<GeneratedSymbol>,
    diagnostics: Vec<BindingDiagnostic>,
}

#[derive(Deserialize)]
struct ProjectSource {
    path: String,
    source: String,
}

#[derive(Serialize)]
struct ProjectBindingOutput {
    modules: Vec<ProjectModuleOutput>,
}

#[derive(Serialize)]
struct ProjectModuleOutput {
    path: String,
    #[serde(flatten)]
    output: BindingOutput,
}

#[derive(Serialize)]
struct BindingDiagnostic {
    code: String,
    message: String,
    file: String,
    start: u32,
    end: u32,
    start_utf16: u32,
    end_utf16: u32,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    notes: Vec<String>,
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
) -> std::result::Result<String, serde_json::Error> {
    let source = SourceFile::new(FileId(1), source_path, source);
    let theme = SourceFile::new(FileId(2), "stillwind.css", theme);
    let result = Compiler::compile_react(&source, &theme);
    serde_json::to_string(&binding_output(result, &source, &theme))
}

fn binding_output(result: CompileResult, source: &SourceFile, theme: &SourceFile) -> BindingOutput {
    let code = result.apply_to_source(source).ok();
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
    BindingOutput {
        css: result.css,
        code,
        classes: result.classes,
        symbols: result.symbols,
        diagnostics,
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
) -> std::result::Result<String, String> {
    let input: Vec<ProjectSource> =
        serde_json::from_str(sources_json).map_err(|error| error.to_string())?;
    let sources = input
        .into_iter()
        .enumerate()
        .map(|(index, source)| {
            SourceFile::new(
                FileId(u32::try_from(index + 1).expect("source graph is smaller than u32::MAX")),
                source.path,
                source.source,
            )
        })
        .collect::<Vec<_>>();
    let theme = SourceFile::new(
        FileId(u32::try_from(sources.len() + 1).expect("source graph is smaller than u32::MAX")),
        "stillwind.css",
        theme,
    );
    let results =
        Compiler::compile_react_project(&sources, &theme).map_err(|error| error.to_string())?;
    let modules = sources
        .iter()
        .zip(results)
        .map(|(source, result)| ProjectModuleOutput {
            path: source.path().to_owned(),
            output: binding_output(result, source, &theme),
        })
        .collect();
    serde_json::to_string(&ProjectBindingOutput { modules }).map_err(|error| error.to_string())
}

/// Extracts the versioned cross-module summary for a React source module.
///
/// # Errors
///
/// Returns an error only when serializing the summary fails.
pub fn summarize_react_json(
    source: &str,
    source_path: &str,
) -> std::result::Result<String, serde_json::Error> {
    let source = SourceFile::new(FileId(1), source_path, source);
    serde_json::to_string(&Compiler::analyze_javascript(&source).summary)
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
pub fn compile_react(source: String, source_path: Option<String>, theme: String) -> Result<String> {
    compile_react_json(
        &source,
        source_path.as_deref().unwrap_or("Component.tsx"),
        &theme,
    )
    .map_err(|error| Error::from_reason(error.to_string()))
}

/// Compiles a resolved React module graph in one native invocation.
///
/// # Errors
///
/// Returns a JavaScript error for invalid input or an unresolved graph edge.
#[napi(js_name = "compileReactProject")]
pub fn compile_react_project(sources_json: String, theme: String) -> Result<String> {
    compile_react_project_json(&sources_json, &theme).map_err(Error::from_reason)
}

/// Returns the versioned Stillwind module summary used for cross-module values.
///
/// # Errors
///
/// Returns a JavaScript error if the summary cannot be serialized.
#[napi(js_name = "summarizeReact")]
pub fn summarize_react(source: String, source_path: Option<String>) -> Result<String> {
    summarize_react_json(&source, source_path.as_deref().unwrap_or("Component.tsx"))
        .map_err(|error| Error::from_reason(error.to_string()))
}

/// Generates the TypeScript vocabulary augmentation for one CSS-first theme.
#[napi(js_name = "generateReactTypes")]
pub fn generate_react_types(theme: String) -> Result<String> {
    let source = SourceFile::new(FileId(2), "stillwind.css", theme);
    Compiler::generate_react_types(&source).map_err(|error| Error::from_reason(error.to_string()))
}
