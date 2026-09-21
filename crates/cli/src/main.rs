//! Bundler-facing command line adapter for the Stillwind compiler.
//!
//! The CLI owns filesystem and package resolution for command-line compilation: it
//! reads the source and theme files, resolves CSS imports through `oxc-resolver`, and
//! rebases resource URLs relative to the entry. Everything downstream is composed from
//! the library crates in the same order a host would use them: scan, finalize without
//! checker facts, compile, render, and print one JSON object on stdout.
//!
//! It is a debugging and acceptance adapter, not a bundler integration: no watcher, no
//! module graph, no caching. Diagnostics and lint findings are flattened to byte ranges
//! in the emitted JSON, because a CLI consumer has no way to use the labeled form.

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

use std::collections::BTreeMap;
use std::sync::Arc;
use std::{env, fs};

use serde::Serialize;

use stillwind_compiler::{
    CompilerOptions, GeneratedClass, Identifiers, ModuleCompiler, ModuleResourceReference,
    StylesheetRenderer,
};
use stillwind_design_system::{DesignSystemSnapshot, LintSeverity};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceError, SourceFile};

mod arguments;
mod loading;

use arguments::{ArgumentError, argument, has_flag, optional_argument};
use loading::{FileLoadError, load_design_system, read_source};

/// Everything the command can fail with.
///
/// Variants wrap the underlying error, so the message a user sees is the specific
/// failure rather than a generic command error.
#[derive(Debug, thiserror::Error)]
enum CommandError {
    /// The source or theme could not be read as a valid buffer.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// No command word was given.
    #[error("expected `compile` command")]
    MissingCommand,
    /// The command word is not one this binary implements.
    #[error("unknown command `{command}`; expected `compile`")]
    UnknownCommand { command: String },
    /// A required argument was absent or missing its value.
    #[error("{0}")]
    Argument(#[from] ArgumentError),
    /// A source, theme or imported CSS file could not be loaded.
    #[error("{0}")]
    File(#[from] FileLoadError),
    /// The theme path could not be made absolute.
    #[error("cannot resolve theme {path}: {source}")]
    ThemePath {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// Design-system options or the loaded design system are invalid.
    #[error("{0}")]
    Options(#[from] CompileErrorKind),
    /// The result could not be serialized.
    #[error("{0}")]
    Output(#[from] serde_json::Error),
}

/// The JSON object written to stdout for a successful compile.
#[derive(Serialize)]
struct CliOutput {
    /// Rendered stylesheet, with resource placeholders already rebased.
    css: String,
    /// Rewritten module source, or `None` when replacement failed.
    code: Option<String>,
    /// Generated classes, for a caller that wants names without the CSS.
    classes: Vec<GeneratedClass>,
    /// Compilation errors, flattened to byte ranges.
    diagnostics: Vec<DiagnosticOutput>,
    /// Suppressible lint findings, flattened the same way.
    lint_findings: Vec<LintFindingOutput>,
    /// Effective lint policy, keyed by rule name.
    lint_policy: BTreeMap<String, LintSeverity>,
}

/// One diagnostic with its primary label reduced to offsets.
#[derive(Serialize)]
struct DiagnosticOutput {
    /// Stable compiler code.
    code: String,
    /// Human-readable message.
    message: String,
    /// Start offset of the first label, in the accepted source.
    start: u32,
    /// End offset, exclusive.
    end: u32,
}

/// One lint finding with its primary label reduced to offsets.
#[derive(Serialize)]
struct LintFindingOutput {
    /// Suppressible rule name.
    rule: String,
    /// Human-readable message.
    message: String,
    /// Start offset of the first label, in the accepted source.
    start: u32,
    /// End offset, exclusive.
    end: u32,
}

/// Rewrites emitted resource URLs to be relative to the theme's directory.
///
/// Both the design system's own resources and the module's resource references carry a
/// placeholder per resource; each placeholder is replaced by the path of the target
/// relative to the entry, with `/` separators so the result is usable from any
/// platform. The replacement is a raw URL, not a CSS string literal, so the JSON
/// escaping used to quote it is stripped again before substitution.
fn rebase_resources(
    mut css: String,
    theme: &SourceFile,
    snapshot: &DesignSystemSnapshot,
    resources: &[ModuleResourceReference],
) -> Result<String, CommandError> {
    let entry_path =
        std::path::absolute(theme.path()).map_err(|source| CommandError::ThemePath {
            path: theme.path().into(),
            source,
        })?;
    let entry_directory = entry_path.parent().unwrap_or(std::path::Path::new("."));
    let snapshot = snapshot.sources();
    let resources = snapshot
        .resources
        .iter()
        .map(|resource| (&resource.placeholder, &resource.owner, &resource.specifier))
        .chain(
            resources
                .iter()
                .map(|resource| (&resource.placeholder, &resource.owner, &resource.specifier)),
        );
    for (placeholder, owner, specifier) in resources {
        let owner_path = std::path::absolute(owner).map_err(|source| CommandError::ThemePath {
            path: owner.clone(),
            source,
        })?;
        let owner_directory = owner_path.parent().unwrap_or(entry_directory);
        let target = owner_directory.join(specifier);
        let rebased = pathdiff::diff_paths(&target, entry_directory).unwrap_or(target);
        let url = rebased.to_string_lossy().replace('\\', "/");
        let quoted = serde_json::to_string(&url)?;
        css = css.replace(placeholder, &quoted[1..quoted.len() - 1]);
    }
    Ok(css)
}

/// Runs one `compile` command and prints its result.
///
/// Source and theme may be given as text or as paths; a text theme is named
/// `stillwind.css` because diagnostics and resource rebasing need a path. Checker facts
/// are deliberately absent, so this path exercises the frontend without a TypeScript
/// checker.
fn run() -> Result<(), CommandError> {
    let command = env::args().nth(1).ok_or(CommandError::MissingCommand)?;
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
        use_css_if: has_flag("--use-css-if"),
        ..CompilerOptions::default()
    };
    options.validate()?;
    let snapshot = load_design_system(&theme, options.design_system())?;
    let analysis =
        OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source.clone())), None);
    let module = ModuleCompiler::new(snapshot.clone(), options.clone()).compile(&analysis);
    let result = StylesheetRenderer::new(&snapshot, &options, &Identifiers::builtin())
        .render_compilation(&module, source.full_span());
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic
                .labels
                .first()
                .map_or(source.full_span(), |label| label.span);
            DiagnosticOutput {
                code: diagnostic.code().to_owned(),
                message: diagnostic.to_string(),
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
                rule: finding.code().to_owned(),
                message: finding.to_string(),
                start: span.start.0,
                end: span.end.0,
            }
        })
        .collect();
    let css = rebase_resources(result.css, &theme, &snapshot, &result.resources)?;
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

/// Prints the error and usage to stderr and exits with status 2.
fn main() {
    if let Err(error) = run() {
        eprintln!("stillwind: {error}");
        eprintln!(
            "usage: stillwind compile --source <component.tsx> --theme <stillwind.css> [--prefix <--sw->] [--tailwind-prefix <--tw->] [--use-css-if]"
        );
        std::process::exit(2);
    }
}
