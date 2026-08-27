//! Bundler-facing command line adapter for the shorth compiler.

use std::{env, fs};

use serde::Serialize;
use shorth_core::{Compiler, GeneratedClass};
use shorth_source::{FileId, SourceFile};

#[derive(Serialize)]
struct CliOutput {
    css: String,
    code: Option<String>,
    classes: Vec<GeneratedClass>,
    diagnostics: Vec<DiagnosticOutput>,
}

#[derive(Serialize)]
struct DiagnosticOutput {
    code: String,
    message: String,
    start: u32,
    end: u32,
}

fn argument(name: &str) -> Result<String, String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments
                .next()
                .ok_or_else(|| format!("missing value for {name}"));
        }
    }
    Err(format!("missing required {name} argument"))
}

fn optional_argument(name: &str) -> Option<String> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == name {
            return arguments.next();
        }
    }
    None
}

fn read_source(path: &str, id: u32) -> Result<SourceFile, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    Ok(SourceFile::new(FileId(id), path, text))
}

fn run() -> Result<(), String> {
    let command = env::args()
        .nth(1)
        .ok_or_else(|| "expected `compile` command".to_owned())?;
    if command != "compile" {
        return Err(format!("unknown command `{command}`; expected `compile`"));
    }
    let source = if let Some(text) = optional_argument("--source-text") {
        let path = optional_argument("--source-path").unwrap_or_else(|| "Play.tsx".to_owned());
        SourceFile::new(FileId(1), path, text)
    } else {
        read_source(&argument("--source")?, 1)?
    };
    let theme = if let Some(text) = optional_argument("--theme-text") {
        SourceFile::new(FileId(2), "shorth.css", text)
    } else {
        read_source(&argument("--theme")?, 2)?
    };
    let result = Compiler::compile_react(&source, &theme);
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            DiagnosticOutput {
                code: diagnostic.code.to_owned(),
                message: diagnostic.message.clone(),
                start: span.start.0,
                end: span.end.0,
            }
        })
        .collect();
    let code = result.apply_to_source(&source).ok();
    let output = CliOutput {
        css: result.css,
        code,
        classes: result.classes,
        diagnostics,
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("shorth: {error}");
        eprintln!("usage: shorth compile --source <component.tsx> --theme <shorth.css>");
        std::process::exit(2);
    }
}
