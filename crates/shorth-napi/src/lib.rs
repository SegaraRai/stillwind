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
    let code = result.apply_to_source(&source).ok();
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
    serde_json::to_string(&BindingOutput {
        css: result.css,
        code,
        classes: result.classes,
        diagnostics,
    })
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

/// Generates the TypeScript vocabulary augmentation for one CSS-first theme.
#[napi(js_name = "generateReactTypes")]
pub fn generate_react_types(theme: String) -> Result<String> {
    let source = SourceFile::new(FileId(2), "stillwind.css", theme);
    Compiler::generate_react_types(&source).map_err(|error| Error::from_reason(error.to_string()))
}
