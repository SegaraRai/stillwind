//! Node-API surface for running the Shorth compiler inside Vite.

use std::collections::{BTreeMap, BTreeSet};

use napi::{Error, Result};
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use shorth_core::{CompileResult, Compiler, CompilerOptions, GeneratedClass, GeneratedSymbol};
use shorth_semantic::{AssetMetadata, IdentityId, TypeFact};
use shorth_source::{FileId, SourceFile};

#[derive(Serialize)]
struct BindingOutput {
    css: String,
    code: Option<String>,
    mappings: Vec<BindingMapping>,
    classes: Vec<GeneratedClass>,
    symbols: Vec<GeneratedSymbol>,
    diagnostics: Vec<BindingDiagnostic>,
}

#[derive(Serialize)]
struct BindingMapping {
    original_start: u32,
    original_end: u32,
    generated_start: u32,
    generated_end: u32,
}

#[derive(Deserialize)]
struct ProjectSource {
    path: String,
    source: String,
    #[serde(default)]
    type_facts: Vec<TypeFact>,
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
    compile_react_json_with_options(source, source_path, theme, &CompilerOptions::default())
}

pub fn compile_react_json_with_options(
    source: &str,
    source_path: &str,
    theme: &str,
    options: &CompilerOptions,
) -> std::result::Result<String, serde_json::Error> {
    let source = SourceFile::new(FileId(1), source_path, source);
    let theme = SourceFile::new(FileId(2), "stillwind.css", theme);
    let result = Compiler::compile_react_with_options(&source, &theme, options);
    serde_json::to_string(&binding_output(result, &source, &theme))
}

fn binding_output(result: CompileResult, source: &SourceFile, theme: &SourceFile) -> BindingOutput {
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
    BindingOutput {
        css: result.css,
        code,
        mappings,
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
    compile_react_project_json_with_options(sources_json, theme, &CompilerOptions::default())
}

pub fn compile_react_project_json_with_options(
    sources_json: &str,
    theme: &str,
    options: &CompilerOptions,
) -> std::result::Result<String, String> {
    let input: Vec<ProjectSource> =
        serde_json::from_str(sources_json).map_err(|error| error.to_string())?;
    let sources = input
        .into_iter()
        .enumerate()
        .map(|(index, source)| {
            (
                SourceFile::new(
                    FileId(
                        u32::try_from(index + 1).expect("source graph is smaller than u32::MAX"),
                    ),
                    source.path,
                    source.source,
                ),
                source.type_facts,
            )
        })
        .collect::<Vec<_>>();
    let theme = SourceFile::new(
        FileId(u32::try_from(sources.len() + 1).expect("source graph is smaller than u32::MAX")),
        "stillwind.css",
        theme,
    );
    let assets = sources
        .iter()
        .flat_map(|(source, _)| Compiler::analyze_javascript(source).summary.assets)
        .map(|asset| (asset.identity, asset.metadata))
        .collect::<BTreeMap<IdentityId, AssetMetadata>>();
    let mut results = sources
        .iter()
        .map(|(source, type_facts)| {
            Compiler::compile_react_with_type_facts_and_options(
                source, &theme, type_facts, &assets, options,
            )
        })
        .collect::<Vec<_>>();
    let mut emitted_css_rules = BTreeSet::new();
    for result in &mut results {
        result.module_css = dedupe_css_rules(&result.module_css, &mut emitted_css_rules);
    }
    let modules = sources
        .iter()
        .zip(results)
        .map(|((source, _), mut result)| {
            // Project consumers assemble one shared stylesheet from these
            // fragments instead of repeating theme/base layers per module.
            result.css.clone_from(&result.module_css);
            ProjectModuleOutput {
                path: source.path().to_owned(),
                output: binding_output(result, source, &theme),
            }
        })
        .collect();
    serde_json::to_string(&ProjectBindingOutput { modules }).map_err(|error| error.to_string())
}

fn dedupe_css_rules(css: &str, emitted: &mut BTreeSet<String>) -> String {
    let bytes = css.as_bytes();
    let mut output = String::new();
    let mut start = 0;
    while start < bytes.len() {
        while start < bytes.len() && bytes[start].is_ascii_whitespace() {
            start += 1;
        }
        if start == bytes.len() {
            break;
        }
        let mut index = start;
        let mut depth = 0_u32;
        let mut quote = None;
        let mut escaped = false;
        let mut comment = false;
        let mut end = bytes.len();
        while index < bytes.len() {
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();
            if comment {
                if byte == b'*' && next == Some(b'/') {
                    comment = false;
                    index += 2;
                    continue;
                }
            } else if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == delimiter {
                    quote = None;
                }
            } else if byte == b'/' && next == Some(b'*') {
                comment = true;
                index += 2;
                continue;
            } else if byte == b'\'' || byte == b'"' {
                quote = Some(byte);
            } else if byte == b'{' {
                depth += 1;
            } else if byte == b'}' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = index + 1;
                    break;
                }
            } else if byte == b';' && depth == 0 {
                end = index + 1;
                break;
            }
            index += 1;
        }
        let rule = css[start..end].trim();
        if !rule.is_empty() && emitted.insert(rule.to_owned()) {
            output.push_str(rule);
        }
        start = end;
    }
    output
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
pub fn compile_react(
    source: String,
    source_path: Option<String>,
    theme: String,
    options_json: Option<String>,
) -> Result<String> {
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
) -> Result<String> {
    let options = parse_options(options_json.as_deref())?;
    compile_react_project_json_with_options(&sources_json, &theme, &options)
        .map_err(Error::from_reason)
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
) -> Result<String> {
    let options = parse_options(options_json.as_deref())?;
    let theme = SourceFile::new(FileId(1), "stillwind.css", theme);
    Compiler::render_react_stylesheet(&theme, &module_css, &options)
        .map_err(|error| Error::from_reason(error.to_string()))
}

fn parse_options(options_json: Option<&str>) -> Result<CompilerOptions> {
    let options = options_json.map_or_else(
        || Ok(CompilerOptions::default()),
        |value| serde_json::from_str(value).map_err(|error| Error::from_reason(error.to_string())),
    )?;
    options.validate().map_err(Error::from_reason)?;
    Ok(options)
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
