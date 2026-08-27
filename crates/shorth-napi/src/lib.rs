//! Node-API surface for running the Shorth compiler inside Vite.

use napi::{Error, Result};
use napi_derive::napi;
use serde::Serialize;
use shorth_core::{Compiler, GeneratedClass};
use shorth_source::{FileId, SourceFile};

#[derive(Serialize)]
struct BindingOutput {
    css: String,
    code: Option<String>,
    classes: Vec<GeneratedClass>,
    diagnostics: Vec<BindingDiagnostic>,
}

#[derive(Serialize)]
struct BindingDiagnostic {
    code: String,
    message: String,
    start: u32,
    end: u32,
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
    let theme = SourceFile::new(FileId(2), "shorth.css", theme);
    let result = Compiler::compile_react(&source, &theme);
    let code = result.apply_to_source(&source).ok();
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            BindingDiagnostic {
                code: diagnostic.code.to_owned(),
                message: diagnostic.message.clone(),
                start: span.start.0,
                end: span.end.0,
            }
        })
        .collect();
    serde_json::to_string(&BindingOutput {
        css: result.css,
        code,
        classes: result.classes,
        diagnostics,
    })
}

/// Native Node entrypoint used by `@shorth/vite`.
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
