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
use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveError, ResolveOptions, Resolver};

use stillwind_design_system::{
    CssFileRequest, DesignSystemLoadError, DesignSystemLoadStep, DesignSystemLoader,
    DesignSystemOptions, DesignSystemSnapshot,
};
use stillwind_source::{FileId, SourceError, SourceFile};

/// Failure to read or resolve a file the loader asked for.
#[derive(Debug, thiserror::Error)]
pub(super) enum FileLoadError {
    /// The text was read but is not a valid source buffer.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// The file could not be read, including when it does not exist.
    #[error("cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A CSS specifier could not be resolved from its importer.
    #[error("cannot resolve {} imported by {}: {source}", .request.specifier, .request.importer.0)]
    Resolve {
        request: CssFileRequest,
        #[source]
        source: Box<ResolveError>,
    },
    /// The design system rejected the loaded files.
    #[error("{0}")]
    Load(#[from] DesignSystemLoadError),
}

/// Reads a file as a source buffer with the given file id.
///
/// The path is stored verbatim as the display path; it is not canonicalized here, so
/// diagnostics and normalized module paths reflect what the caller passed.
pub(super) fn read_source(path: &str, id: u32) -> Result<SourceFile, FileLoadError> {
    let text = fs::read_to_string(path).map_err(|error| FileLoadError::Read {
        path: path.into(),
        source: error,
    })?;
    Ok(SourceFile::new(FileId(id), path, text)?)
}

/// Drives the design-system loader to completion over the real filesystem.
///
/// The entry file is supplied directly, because the caller has already read it; every
/// further request is resolved and read here. The loop serves one request per step
/// rather than batching, which keeps error attribution exact.
pub(super) fn load_design_system(
    theme: &SourceFile,
    options: DesignSystemOptions,
) -> Result<DesignSystemSnapshot, FileLoadError> {
    let resolver = Resolver::new(ResolveOptions {
        condition_names: vec!["style".into(), "import".into(), "default".into()],
        extensions: vec![".css".into()],
        main_fields: vec!["style".into(), "main".into()],
        ..ResolveOptions::default()
    });
    let mut loader = DesignSystemLoader::new(options);
    let mut step = loader.provide(0, theme.path().into(), theme.text().into());
    loop {
        match step {
            DesignSystemLoadStep::Complete(result) => {
                return result.map_err(FileLoadError::Load);
            }
            DesignSystemLoadStep::NeedFile(requests) => {
                let request = &requests[0];
                let directory = Path::new(request.importer.0.as_ref())
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
                step = loader.provide(
                    request.id,
                    path.to_string_lossy().as_ref().into(),
                    content.into(),
                );
            }
        }
    }
}
