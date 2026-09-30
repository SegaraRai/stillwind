//! Filesystem and package-resolution boundaries of the CLI.
//!
//! The design system itself never resolves paths: it asks for files by request, and
//! this module answers. CSS specifiers are resolved with the `style`, `import` and
//! `default` export conditions and a `.css` extension, so a package can publish a
//! `style` condition the way a bundler would see it.
//!
//! Requests are served one at a time, in the order the loader asks, so a missing file
//! is reported with the importer that requested it.

use std::fs;
use std::path::{Path, PathBuf, absolute};
use std::sync::Arc;

use oxc_resolver::{ResolveError, ResolveOptions, Resolver};
use url::Url;

use stillwind_design_system::{
    CssFileRequest, DesignSystemError, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader, DesignSystemOptions, DesignSystemSnapshot,
};
use stillwind_diagnostics::{AggregateError, LintSettings, Reporter};
use stillwind_source::{
    SourceError, SourceFile, SourceRegistry, SourceRegistryError, SourceUri, SourceUriError,
};

/// Failure to read or resolve a file the loader asked for.
#[derive(Debug, thiserror::Error)]
pub(super) enum FileLoadError {
    /// The text was read but is not a valid source buffer.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// A source revision could not be registered.
    #[error("{0}")]
    Registry(#[from] SourceRegistryError),
    /// A diagnostic could not be resolved against accepted revisions.
    #[error("{0}")]
    Aggregate(#[from] AggregateError),
    /// The filesystem path could not be represented as a file URI.
    #[error("cannot form file URI for {}", .0.display())]
    FileUri(PathBuf),
    /// A file URI did not pass source identity validation.
    #[error("{0}")]
    Uri(#[from] SourceUriError),
    /// The file could not be read, including when it does not exist.
    #[error("cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A CSS specifier could not be resolved from its importer.
    #[error("cannot resolve {} imported by {}: {source}", .request.specifier, .request.importer.as_str())]
    Resolve {
        request: CssFileRequest,
        #[source]
        source: Box<ResolveError>,
    },
    /// The design system rejected the loaded files.
    #[error("{0}")]
    Load(#[from] DesignSystemLoadError),
    /// A source-resolved stylesheet failure.
    #[error("{0}")]
    Reported(String),
}

/// Reads a file as a source buffer with its host-owned document URI.
pub(super) fn read_source(
    path: &str,
    sources: &mut SourceRegistry,
) -> Result<Arc<SourceFile>, FileLoadError> {
    let text = fs::read_to_string(path).map_err(|error| FileLoadError::Read {
        path: path.into(),
        source: error,
    })?;
    let uri = file_uri(Path::new(path))?;
    Ok(sources.register(Some(uri.clone()), Some(uri.as_str().into()), text)?)
}

/// Forms the absolute document URI for a filesystem source.
pub(super) fn file_uri(path: &Path) -> Result<SourceUri, FileLoadError> {
    let absolute = absolute(path).map_err(|source| FileLoadError::Read {
        path: path.into(),
        source,
    })?;
    let url = Url::from_file_path(&absolute).map_err(|()| FileLoadError::FileUri(absolute))?;
    Ok(SourceUri::parse(url.as_str())?)
}

/// Drives the design-system loader to completion over the real filesystem.
///
/// The entry file is supplied directly, because the caller has already read it; every
/// further request is resolved and read here. The loop serves one request per step
/// rather than batching, which keeps error attribution exact.
pub(super) fn load_design_system(
    theme: &SourceFile,
    theme_name: &str,
    options: DesignSystemOptions,
    sources: &mut SourceRegistry,
) -> Result<DesignSystemSnapshot, FileLoadError> {
    let resolver = Resolver::new(ResolveOptions {
        condition_names: vec!["style".into(), "import".into(), "default".into()],
        extensions: vec![".css".into()],
        main_fields: vec!["style".into(), "main".into()],
        ..ResolveOptions::default()
    });
    let mut loader = DesignSystemLoader::new(options, sources.fork_empty());
    let mut step = loader.provide(
        0,
        theme_name.into(),
        Some(
            theme
                .uri()
                .ok_or_else(|| FileLoadError::FileUri(theme_name.into()))?
                .clone(),
        ),
        theme.source_map_source().map(Into::into),
        theme.text().into(),
    );
    loop {
        match step {
            DesignSystemLoadStep::Complete(result) => {
                let snapshot = match result {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        let Some(root) = loader.root_source() else {
                            return Err(FileLoadError::Load(error));
                        };
                        let compiled = DesignSystemError::Load(error.clone())
                            .into_compile_error(root.full_span());
                        let text = Reporter::collect(
                            std::slice::from_ref(&compiled),
                            &[],
                            loader.sources(),
                            LintSettings::default(),
                        )?
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.display_text())
                        .collect::<Vec<_>>()
                        .join("\n");
                        return Err(FileLoadError::Reported(text));
                    }
                };
                for source in &snapshot.sources().sources {
                    sources.retain(source.source.clone())?;
                }
                return Ok(snapshot);
            }
            DesignSystemLoadStep::NeedFile(requests) => {
                let request = &requests[0];
                let directory = Path::new(request.importer.as_str())
                    .parent()
                    .unwrap_or_else(|| Path::new("."));
                let path = resolver
                    .resolve(directory, &request.specifier)
                    .map_err(|error| FileLoadError::Resolve {
                        request: request.clone(),
                        source: Box::new(error),
                    })?
                    .full_path();
                let content = fs::read_to_string(&path).map_err(|error| FileLoadError::Read {
                    path: path.clone(),
                    source: error,
                })?;
                let uri = file_uri(&path)?;
                step = loader.provide(
                    request.id,
                    path.to_string_lossy().as_ref().into(),
                    Some(uri.clone()),
                    Some(uri.as_str().into()),
                    content.into(),
                );
            }
        }
    }
}
