//! Bundler-facing command line adapter for the Stillwind compiler.
//!
//! The CLI owns filesystem and package resolution for command-line compilation: it
//! reads the source and theme files, resolves CSS imports through `oxc-resolver`, and
//! rebases resource URLs relative to the entry. Everything downstream is composed from
//! the library crates in the same order a host would use them: scan, finalize without
//! checker facts, compile, render, and print one JSON object on stdout.
//!
//! It is a debugging and acceptance adapter, not a bundler integration: no watcher, no
//! module graph, no caching. Diagnostics and lint findings retain their resolved
//! source locations, related labels, and fix actions in the emitted JSON.

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

use std::{env, fs};

use serde::Serialize;

use stillwind_compiler::{
    CompilerOptions, GeneratedClass, Identifiers, ModuleCompiler, ModuleResourceReference,
    StylesheetRenderer, TransformError,
};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{
    AggregateError, CompileDiagnostic, LintSettings, Reporter, ResolvedDiagnostic,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{ModuleId, SourceError, SourceRegistry, SourceRegistryError, SourceUri};

mod arguments;
mod loading;

use arguments::{ArgumentError, argument, has_flag, optional_argument};
use loading::{FileLoadError, load_design_system, read_source};

/// A CLI invocation error and its underlying cause.
///
/// Variants wrap the underlying error, so the message a user sees is the specific
/// failure rather than a generic command error.
#[derive(Debug, thiserror::Error)]
enum CommandError {
    /// The source or theme could not be read as a valid buffer.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// A source revision or diagnostic range could not be found.
    #[error("{0}")]
    Registry(#[from] SourceRegistryError),
    /// A diagnostic batch contained conflicting revisions or edits.
    #[error("{0}")]
    Aggregate(#[from] AggregateError),
    /// Source replacement could not be rendered after compilation.
    #[error("{0}")]
    Transform(#[from] TransformError),
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
    /// A compiled resource belongs to a module absent from this one-source invocation.
    #[error("cannot resolve resource from unknown module {0}")]
    ResourceOwner(String),
    /// Design-system options or the loaded design system are invalid.
    #[error("{0}")]
    Options(#[from] CompileDiagnostic),
    /// The result could not be serialized.
    #[error("{0}")]
    Output(#[from] serde_json::Error),
}

/// The JSON object written to stdout for a successful compile.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliOutput {
    /// Rendered stylesheet, with resource placeholders already rebased.
    css: String,
    /// Rewritten module source, or `None` when compilation reported diagnostics.
    code: Option<String>,
    /// Generated classes, for a caller that wants names without the CSS.
    classes: Vec<GeneratedClass>,
    /// Compilation errors with resolved source locations.
    diagnostics: Vec<ResolvedDiagnostic>,
    /// Suppressible lint findings with resolved source locations.
    lint_findings: Vec<ResolvedDiagnostic>,
    /// Effective lint policy, keyed by rule name.
    lint_settings: LintSettings,
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
    theme_name: &str,
    source_path: &str,
    module_id: &ModuleId,
    snapshot: &DesignSystemSnapshot,
    resources: &[ModuleResourceReference],
) -> Result<String, CommandError> {
    for resource in resources {
        if &resource.owner != module_id {
            return Err(CommandError::ResourceOwner(resource.owner.to_string()));
        }
    }
    let entry_path = std::path::absolute(theme_name).map_err(|source| CommandError::ThemePath {
        path: theme_name.into(),
        source,
    })?;
    let entry_directory = entry_path.parent().unwrap_or(std::path::Path::new("."));
    let snapshot = snapshot.sources();
    let resources = snapshot
        .resources
        .iter()
        .map(|resource| {
            (
                &resource.placeholder,
                resource.source_name.as_str(),
                &resource.specifier,
            )
        })
        .chain(
            resources
                .iter()
                .map(|resource| (&resource.placeholder, source_path, &resource.specifier)),
        );
    for (placeholder, owner, specifier) in resources {
        let owner_path = std::path::absolute(owner).map_err(|source| CommandError::ThemePath {
            path: owner.to_owned(),
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
    let mut sources = SourceRegistry::default();
    let (source, source_path) = if let Some(text) = optional_argument("--source-text") {
        let path = optional_argument("--source-path").unwrap_or_else(|| "Play.tsx".to_owned());
        let source = sources.register(
            Some(
                SourceUri::parse("stillwind-cli:source")
                    .map_err(|error| CommandError::File(FileLoadError::Uri(error)))?,
            ),
            None,
            text,
        )?;
        (source, path)
    } else {
        let path = argument("--source")?;
        (read_source(&path, &mut sources)?, path)
    };
    let (theme, theme_name) = if let Some(text) = optional_argument("--theme-text") {
        let theme = sources.register(
            Some(
                SourceUri::parse("stillwind-cli:theme")
                    .map_err(|error| CommandError::File(FileLoadError::Uri(error)))?,
            ),
            None,
            text,
        )?;
        (theme, "stillwind.css".to_owned())
    } else {
        let input = argument("--theme")?;
        let path = fs::canonicalize(&input).map_err(|source| CommandError::ThemePath {
            path: input,
            source,
        })?;
        (
            read_source(&path.to_string_lossy(), &mut sources)?,
            path.to_string_lossy().into_owned(),
        )
    };
    let module_id = ModuleId::from(source_path.as_str());
    let options = CompilerOptions {
        prefix: optional_argument("--prefix").unwrap_or_else(|| "--sw-".to_owned()),
        tailwind_prefix: optional_argument("--tailwind-prefix")
            .unwrap_or_else(|| "--tw-".to_owned()),
        forbid_tailwind_relationships: has_flag("--forbid-tailwind-relationships"),
        use_css_if: has_flag("--use-css-if"),
        ..CompilerOptions::default()
    };
    options.validate()?;
    let snapshot = load_design_system(&theme, &theme_name, options.design_system(), &mut sources)?;
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source.clone(),
            module_id.clone(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let module = ModuleCompiler::new(snapshot.clone(), options.clone())
        .compile(&analysis, &Identifiers::default());
    let result = StylesheetRenderer::new(&snapshot, &options, &Identifiers::default())
        .render_compilation(&module, source.full_span());
    let reports = Reporter::collect(
        &result.diagnostics,
        &result.lint_findings,
        &sources,
        result.lint_settings.clone(),
    )?;
    let diagnostic_count = result.diagnostics.len();
    let mut reports = reports.diagnostics;
    let lint_findings = reports.split_off(diagnostic_count);
    let diagnostics = reports;
    let code = if result.diagnostics.is_empty() {
        Some(result.apply_to_source(&source)?)
    } else {
        None
    };
    let css = rebase_resources(
        result.css,
        &theme_name,
        &source_path,
        &module_id,
        &snapshot,
        &result.resources,
    )?;
    let output = CliOutput {
        css,
        code,
        classes: result.classes,
        diagnostics,
        lint_findings,
        lint_settings: result.lint_settings,
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
