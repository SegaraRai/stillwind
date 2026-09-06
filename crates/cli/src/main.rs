//! Bundler-facing command line adapter for the Stillwind compiler.

use std::{env, fs};

use serde::Serialize;
use stillwind_core::{Compiler, CompilerOptions, GeneratedClass};
use stillwind_source::{FileId, SourceFile};

#[derive(Serialize)]
struct CliOutput {
    css: String,
    code: Option<String>,
    classes: Vec<GeneratedClass>,
    diagnostics: Vec<DiagnosticOutput>,
    lint_findings: Vec<LintFindingOutput>,
    lint_policy: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
struct DiagnosticOutput {
    code: String,
    message: String,
    start: u32,
    end: u32,
}

#[derive(Serialize)]
struct LintFindingOutput {
    rule: String,
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

fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|argument| argument == name)
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
        SourceFile::new(FileId(2), "stillwind.css", text)
    } else {
        read_source(&argument("--theme")?, 2)?
    };
    let options = CompilerOptions {
        prefix: optional_argument("--prefix").unwrap_or_else(|| "--sw-".to_owned()),
        tailwind_prefix: optional_argument("--tailwind-prefix")
            .unwrap_or_else(|| "--tw-".to_owned()),
        forbid_tailwind_relationships: has_flag("--forbid-tailwind-relationships"),
    };
    options.validate()?;
    let result = Compiler::compile_react_with_options(&source, &theme, &options);
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
    let lint_findings = result
        .lint_findings
        .iter()
        .map(|finding| {
            let span = finding
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            LintFindingOutput {
                rule: finding.rule.to_owned(),
                message: finding.message.clone(),
                start: span.start.0,
                end: span.end.0,
            }
        })
        .collect();
    let output = CliOutput {
        css: result.css,
        code,
        classes: result.classes,
        diagnostics,
        lint_findings,
        lint_policy: result.lint_policy,
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("stillwind: {error}");
        eprintln!(
            "usage: stillwind compile --source <component.tsx> --theme <stillwind.css> [--prefix <--sw->] [--tailwind-prefix <--tw->]"
        );
        std::process::exit(2);
    }
}
