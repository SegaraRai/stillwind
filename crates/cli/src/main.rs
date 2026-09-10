//! Bundler-facing command line adapter for the Stillwind compiler.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

use std::{env, fs, sync::Arc};

use serde::Serialize;

use stillwind_core::{
    CompilerOptions, CompilerOptionsError, GeneratedClass, ModuleCompiler, OxcSourceAnalyzer,
    SourceAnalyzer,
};
use stillwind_source::{FileId, SourceError, SourceFile};

mod arguments;
mod loading;

use arguments::{ArgumentError, argument, has_flag, optional_argument};
use loading::{FileLoadError, load_design_system, read_source};

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

#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error("{0}")]
    Source(#[from] SourceError),
    #[error("expected `compile` command")]
    MissingCommand,
    #[error("unknown command `{command}`; expected `compile`")]
    UnknownCommand { command: String },
    #[error("{0}")]
    Argument(#[from] ArgumentError),
    #[error("{0}")]
    File(#[from] FileLoadError),
    #[error("cannot resolve theme {path}: {source}")]
    ThemePath {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Options(#[from] CompilerOptionsError),
    #[error("{0}")]
    Output(#[from] serde_json::Error),
}

fn run() -> Result<(), CommandError> {
    let command = env::args()
        .nth(1)
        .ok_or_else(|| CommandError::MissingCommand)?;
    if command != "compile" {
        return Err(CommandError::UnknownCommand { command });
    }
    let source = if let Some(text) = optional_argument("--source-text") {
        let path = optional_argument("--source-path").unwrap_or_else(|| "Play.tsx".to_owned());
        SourceFile::new(FileId(1), path, text)?
    } else {
        read_source(&argument("--source")?, 1)?
    };
    let theme = if let Some(text) = optional_argument("--theme-text") {
        SourceFile::new(FileId(2), "stillwind.css", text)?
    } else {
        let input = argument("--theme")?;
        let path = fs::canonicalize(&input).map_err(|source| CommandError::ThemePath {
            path: input,
            source,
        })?;
        read_source(&path.to_string_lossy(), 2)?
    };
    let options = CompilerOptions {
        prefix: optional_argument("--prefix").unwrap_or_else(|| "--sw-".to_owned()),
        tailwind_prefix: optional_argument("--tailwind-prefix")
            .unwrap_or_else(|| "--tw-".to_owned()),
        forbid_tailwind_relationships: has_flag("--forbid-tailwind-relationships"),
        ..CompilerOptions::default()
    };
    options.validate()?;
    let snapshot = load_design_system(&theme, options)?;
    let analysis =
        OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source.clone())), None);
    let result = ModuleCompiler::new(snapshot.clone())
        .compile(&analysis)
        .to_legacy(&snapshot);
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
    let mut css = result.css;
    let entry_path =
        std::path::absolute(theme.path()).map_err(|source| CommandError::ThemePath {
            path: theme.path().into(),
            source,
        })?;
    let entry_directory = entry_path.parent().unwrap_or(std::path::Path::new("."));
    for resource in &snapshot.sources().resources {
        let owner_path =
            std::path::absolute(&resource.owner).map_err(|source| CommandError::ThemePath {
                path: resource.owner.clone(),
                source,
            })?;
        let owner_directory = owner_path.parent().unwrap_or(entry_directory);
        let target = owner_directory.join(&resource.specifier);
        let rebased = pathdiff::diff_paths(&target, entry_directory).unwrap_or(target);
        let url = rebased.to_string_lossy().replace('\\', "/");
        let quoted = serde_json::to_string(&url)?;
        css = css.replace(&resource.placeholder, &quoted[1..quoted.len() - 1]);
    }
    let output = CliOutput {
        css,
        code,
        classes: result.classes,
        diagnostics,
        lint_findings,
        lint_policy: result.lint_policy,
    };
    println!("{}", serde_json::to_string(&output)?);
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
