//! Drives CSS load requests and retains the completed design-system snapshot.

use napi::{Env, Error};
use napi_derive::napi;

use stillwind_compiler::{CompilerOptions, lint_stylesheets, prepare_canonical_index};
use stillwind_design_system::{
    CssFileInput, CssSourceGraph, DesignSystemError, DesignSystemLoadStep, DesignSystemLoader,
    DesignSystemSnapshot,
};
use stillwind_diagnostics::{LintSettings, LintSurface, LocatedDiagnostic, Reporter};
use stillwind_source::{SourceRegistry, SourceUri};

use super::NativeSessionError;
use crate::editor::NativeEditorQuery;
use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::{BindingReport, report_error};
use crate::stylesheet::{NativeCssLoadStep, NativeCssRequest, NativeCssResource, NativeSkippedCss};

/// One CSS source supplied by the host in one [`NativeDesignSystemLoader::provide`]
/// call.
#[napi(object)]
pub struct NativeCssFileInput {
    /// Request id being answered; `0` is the root source.
    pub id: u32,
    /// The host chooses this name for CSS resolution. The loader compares it as
    /// supplied.
    pub name: String,
    /// Absolute document URI supplied by the host; absent means an internal buffer.
    pub uri: Option<String>,
    /// Exact source name to write into a generated source map.
    pub source_map_source: Option<String>,
    /// Full text of the source.
    pub content: String,
    /// Whether stylesheet lint checks this source. Mark the user's own CSS and leave
    /// package CSS unmarked. Absent means false.
    pub lint: Option<bool>,
    /// Whether the host could not resolve or read the requested file. The loader then
    /// leaves the import out and records it for `invalid-css-import`, and it ignores
    /// the other fields except `id`. Absent means false. The root cannot be unresolved.
    pub unresolved: Option<bool>,
}

/// Loads CSS files supplied by the caller into a design-system snapshot.
///
/// Supply the root source first, then answer each requested ID. The loader reports
/// parse and link errors. If the host cannot resolve or read an imported file, it
/// answers the request with `unresolved: true`. If it cannot read the root, it must
/// report that error and discard the loader.
#[napi]
pub struct NativeDesignSystemLoader {
    loader: DesignSystemLoader,
    options: CompilerOptions,
    snapshot: Option<DesignSystemSnapshot>,
    /// Whether completion prepares the canonical spelling index.
    prepare_canonical: bool,
}

#[napi]
impl NativeDesignSystemLoader {
    /// Creates a loader for the given compiler options.
    #[napi(constructor, strict)]
    pub fn new(options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        Self::with_sources(options, SourceRegistry::default())
    }

    /// Creates an empty loader that shares source identities with this loader and its
    /// sessions. `options` selects compiler settings; omission uses the compiler
    /// defaults.
    #[napi]
    pub fn new_loader(&self, options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        Self::with_sources(options, self.sources().fork_empty())
    }

    /// Adds a `@plugin` request without options after the root's text. Its answer
    /// follows every authored rule, and its definitions are plugin definitions.
    ///
    /// Choose a `specifier` that authored CSS cannot contain, such as one with a NUL
    /// character, so the request cannot be confused with an authored `@plugin`.
    ///
    /// # Errors
    ///
    /// Returns an error after the root has been supplied.
    #[napi]
    pub fn request_trailing_plugin(&mut self, specifier: String) -> napi::Result<()> {
        if self.loader.request_trailing_plugin(specifier.into()) {
            Ok(())
        } else {
            Err(Error::from_reason(
                "requestTrailingPlugin must be called before the root is supplied",
            ))
        }
    }

    /// Answers one pending request by calling [`Self::provide_many`] with one file.
    /// An absent URI marks the CSS as an internal buffer without a reportable location.
    /// `lint` marks the source for stylesheet lint, and an absent value means false.
    #[napi]
    #[allow(clippy::too_many_arguments)] // Node-API exposes the file fields as positional arguments.
    pub fn provide(
        &mut self,
        env: Env,
        id: u32,
        name: String,
        content: String,
        uri: Option<String>,
        source_map_source: Option<String>,
        lint: Option<bool>,
    ) -> napi::Result<NativeCssLoadStep> {
        self.provide_many(
            env,
            vec![NativeCssFileInput {
                id,
                name,
                uri,
                source_map_source,
                content,
                lint,
                unresolved: None,
            }],
        )
    }

    /// Answers any subset of the outstanding requests, in any order.
    ///
    /// Each supplied source is parsed immediately, and its imports appear in the next
    /// step. Other requests can remain unanswered. Unknown or duplicate IDs cause
    /// errors.
    #[napi(strict)]
    pub fn provide_many(
        &mut self,
        env: Env,
        files: Vec<NativeCssFileInput>,
    ) -> napi::Result<NativeCssLoadStep> {
        let files = files
            .into_iter()
            .map(|file| {
                let uri = file
                    .uri
                    .map(SourceUri::parse)
                    .transpose()
                    .map_err(|error| Error::from_reason(error.to_string()))?;
                Ok(CssFileInput {
                    id: file.id,
                    name: file.name.as_str().into(),
                    uri,
                    source_map_source: file.source_map_source.map(Into::into),
                    content: file.content.into(),
                    lint: file.lint.unwrap_or(false),
                    unresolved: file.unresolved.unwrap_or(false),
                })
            })
            .collect::<napi::Result<Vec<_>>>()?;
        match self.loader.provide_many(files) {
            DesignSystemLoadStep::NeedFile(requests) => Ok(NativeCssLoadStep::NeedFile {
                requests: requests.into_iter().map(NativeCssRequest::from).collect(),
            }),
            DesignSystemLoadStep::Complete(Ok(snapshot)) => {
                if self.prepare_canonical {
                    prepare_canonical_index(&snapshot, &self.options);
                }
                let output = NativeCssLoadStep::Complete {
                    revision: snapshot.revision().0.to_string(),
                    resources: snapshot
                        .sources()
                        .resources
                        .iter()
                        .map(|resource| NativeCssResource {
                            placeholder: (resource.placeholder.clone()).into(),
                            source_name: (resource.source_name.as_str().to_owned()).into(),
                            specifier: (resource.specifier.clone()).into(),
                            start: resource.span.start.0,
                            end: resource.span.end.0,
                        })
                        .collect(),
                    sources: snapshot
                        .sources()
                        .sources
                        .iter()
                        .map(|source| source.name.as_str().to_owned())
                        .collect(),
                    lint_sources: snapshot
                        .sources()
                        .sources
                        .iter()
                        .filter(|source| source.lint)
                        .map(|source| source.name.as_str().to_owned())
                        .collect(),
                    skipped: skipped_statements(snapshot.sources()),
                };
                self.snapshot = Some(snapshot);
                Ok(output)
            }
            DesignSystemLoadStep::Complete(Err(error)) => {
                if let Some(root) = self
                    .loader
                    .root_source()
                    .filter(|source| source.uri().is_some())
                {
                    let compiled =
                        DesignSystemError::Load(error.clone()).into_compile_error(root.full_span());
                    let reports = Reporter::collect(
                        std::slice::from_ref(&compiled),
                        &[],
                        self.loader.sources(),
                        LintSettings::default(),
                    )
                    .map_err(|failure| Error::from_reason(failure.to_string()))?
                    .diagnostics
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                    Err(report_error(env, error.to_string(), reports))
                } else {
                    Err(Error::from_reason(error.to_string()))
                }
            }
        }
    }

    /// Returns lint reports for the sources marked with `lint`, using the lint settings
    /// of this loader's options and the stylesheet's own lint configuration.
    ///
    /// The method takes no arguments. Lint settings come only from the options passed
    /// to the constructor, so a host that needs other settings creates another loader.
    ///
    /// Reports and fixes address the marked CSS documents. The analysis runs once for
    /// each design-system revision and compiler options, and later calls reuse it.
    ///
    /// # Errors
    ///
    /// Returns an error while CSS loading is incomplete or when a report range is
    /// invalid.
    #[napi]
    pub fn stylesheet_reports(&self) -> napi::Result<Vec<BindingReport>> {
        stylesheet_reports(&self.completed()?, &self.options, self.loader.sources())
    }

    /// Skips preparing the canonical spelling index when loading completes.
    ///
    /// A host calls this for a design system that only serves as an intermediate load,
    /// such as the load whose theme a later load reuses. Canonical queries still
    /// prepare the index on first use, so results do not change.
    #[napi]
    pub fn defer_canonical_index(&mut self) {
        self.prepare_canonical = false;
    }

    /// Prepares the canonical spelling index of the completed design system, which
    /// loading skips after [`Self::defer_canonical_index`]. Repeated calls reuse the
    /// prepared index. Returns an error while CSS loading is incomplete.
    #[napi]
    pub fn prepare_canonical_index(&self) -> napi::Result<()> {
        prepare_canonical_index(&self.completed()?, &self.options);
        Ok(())
    }

    /// Creates editor queries pinned to this completed design system and its compiler
    /// options. Returns an error while CSS loading is incomplete.
    #[napi]
    pub fn editor_query(&self) -> napi::Result<NativeEditorQuery> {
        Ok(NativeEditorQuery::new(
            self.completed()?,
            self.options.clone(),
        ))
    }
}

impl NativeDesignSystemLoader {
    /// Creates an empty loader whose sources register in `sources`, so their identities
    /// match a session that shares the registry.
    ///
    /// # Errors
    ///
    /// Returns an error when `options` are invalid.
    pub(super) fn with_sources(
        options: Option<NativeCompilerOptions>,
        sources: SourceRegistry,
    ) -> napi::Result<Self> {
        let options = compiler_options(options)?;
        Ok(Self {
            loader: DesignSystemLoader::new(options.design_system(), sources),
            options,
            snapshot: None,
            prepare_canonical: true,
        })
    }

    /// Compiler options that this loader was created with.
    pub(super) fn options(&self) -> &CompilerOptions {
        &self.options
    }

    /// Registry of the sources that this loader received.
    pub(super) fn sources(&self) -> &SourceRegistry {
        self.loader.sources()
    }

    /// Snapshot of a completed load, or an error naming the missing step.
    pub(super) fn completed(&self) -> Result<DesignSystemSnapshot, NativeSessionError> {
        self.snapshot
            .clone()
            .ok_or(NativeSessionError::IncompleteCssLoad)
    }
}

/// Resolves the stylesheet lint of `snapshot` under `options` into reports.
///
/// `sources` must use the file identities of `snapshot`'s sources. Suppression
/// comments are read with stylesheet syntax.
///
/// # Errors
///
/// Returns an error when a source cannot be registered or a report range is invalid.
pub(super) fn stylesheet_reports(
    snapshot: &DesignSystemSnapshot,
    options: &CompilerOptions,
    sources: &SourceRegistry,
) -> napi::Result<Vec<BindingReport>> {
    let failure = |error: &dyn std::fmt::Display| Error::from_reason(error.to_string());
    let lint = lint_stylesheets(snapshot, options);
    let mut registry = sources.fork_empty();
    // Primary labels address marked sources, but a related label can address any
    // source with a URI, such as a later definition in an imported file.
    for source in &snapshot.sources().sources {
        if source.source.uri().is_some() {
            registry
                .retain(source.source.clone())
                .map_err(|error| failure(&error))?;
        }
    }
    let mut reporter =
        Reporter::new(lint.lint_settings.clone()).with_surface(LintSurface::Stylesheet);
    for finding in &lint.findings {
        let located =
            LocatedDiagnostic::from_lint(finding, &registry).map_err(|error| failure(&error))?;
        reporter.report(&located).map_err(|error| failure(&error))?;
    }
    Ok(reporter
        .finish()
        .diagnostics
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Returns the statements that loading skipped, located in their authored sources.
///
/// A record whose span does not address a source of `graph` is left out.
fn skipped_statements(graph: &CssSourceGraph) -> Vec<NativeSkippedCss> {
    graph
        .skipped
        .iter()
        .filter_map(|skipped| {
            let source = graph
                .sources
                .iter()
                .find(|source| source.source.id() == skipped.span.file)?;
            let position = source.source.position(skipped.span.start).ok()?;
            Some(NativeSkippedCss {
                source_name: source.name.as_str().to_owned(),
                start: skipped.span.start.0,
                end: skipped.span.end.0,
                line: position.line,
                column: position.utf16_column,
                rule: if skipped.reason.is_import() {
                    "invalid-css-import"
                } else {
                    "invalid-css-directive"
                }
                .to_owned(),
                message: skipped.reason.to_string(),
            })
        })
        .collect()
}
