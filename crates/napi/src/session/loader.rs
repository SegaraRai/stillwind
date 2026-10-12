//! Drives CSS load requests and retains the completed design-system snapshot.

use napi::{Env, Error};
use napi_derive::napi;

use stillwind_compiler::{CompilerOptions, prepare_canonical_index};
use stillwind_design_system::{
    CssFileInput, DesignSystemError, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_diagnostics::{LintSettings, Reporter};

use stillwind_source::{SourceRegistry, SourceUri};

use crate::editor::NativeEditorQuery;

use crate::options::{NativeCompilerOptions, compiler_options};
use crate::output::report_error;

use crate::stylesheet::{NativeCssLoadStep, NativeCssRequest, NativeCssResource};

use super::NativeSessionError;

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
}

/// Loads CSS files supplied by the caller into a design-system snapshot.
///
/// Supply the root source first, then answer each requested ID. The loader reports
/// parse and link errors. If the host cannot read a file, it must report that error
/// and discard the loader.
#[napi]
pub struct NativeDesignSystemLoader {
    loader: DesignSystemLoader,
    options: CompilerOptions,
    snapshot: Option<DesignSystemSnapshot>,
}

#[napi]
impl NativeDesignSystemLoader {
    /// Creates a loader for the given compiler options.
    #[napi(constructor, strict)]
    pub fn new(options: Option<NativeCompilerOptions>) -> napi::Result<Self> {
        Self::with_sources(options, SourceRegistry::default())
    }

    /// Answers one pending request by calling [`Self::provide_many`] with one file.
    /// An absent URI marks the CSS as an internal buffer without a reportable location.
    #[napi]
    pub fn provide(
        &mut self,
        env: Env,
        id: u32,
        name: String,
        content: String,
        uri: Option<String>,
        source_map_source: Option<String>,
    ) -> napi::Result<NativeCssLoadStep> {
        self.provide_many(
            env,
            vec![NativeCssFileInput {
                id,
                name,
                uri,
                source_map_source,
                content,
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
                })
            })
            .collect::<napi::Result<Vec<_>>>()?;
        match self.loader.provide_many(files) {
            DesignSystemLoadStep::NeedFile(requests) => Ok(NativeCssLoadStep::NeedFile {
                requests: requests
                    .into_iter()
                    .map(|request| NativeCssRequest {
                        id: request.id,
                        importer: request.importer.as_str().to_owned(),
                        specifier: request.specifier,
                    })
                    .collect(),
            }),
            DesignSystemLoadStep::Complete(Ok(snapshot)) => {
                prepare_canonical_index(&snapshot, &self.options);
                let output = NativeCssLoadStep::Complete {
                    revision: snapshot.revision().0.to_string(),
                    resources: snapshot
                        .sources()
                        .resources
                        .iter()
                        .map(|resource| NativeCssResource {
                            placeholder: resource.placeholder.clone(),
                            source_name: resource.source_name.as_str().to_owned(),
                            specifier: resource.specifier.clone(),
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

    /// Creates editor queries pinned to this completed design system and its compiler options.
    /// Returns an error while CSS loading is incomplete.
    #[napi]
    pub fn editor_query(&self) -> napi::Result<NativeEditorQuery> {
        Ok(NativeEditorQuery::new(
            self.completed()?,
            self.options.clone(),
        ))
    }
}

impl NativeDesignSystemLoader {
    pub(super) fn with_sources(
        options: Option<NativeCompilerOptions>,
        sources: SourceRegistry,
    ) -> napi::Result<Self> {
        let options = compiler_options(options)?;
        Ok(Self {
            loader: DesignSystemLoader::new(options.design_system(), sources),
            options,
            snapshot: None,
        })
    }

    pub(super) fn options(&self) -> &CompilerOptions {
        &self.options
    }

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
