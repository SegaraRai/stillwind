use std::fs;
use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveError, ResolveOptions, Resolver};

use stillwind_design_system::{
    CssFileRequest, DesignSystemLoadError, DesignSystemLoadStep, DesignSystemLoader,
    DesignSystemOptions, DesignSystemSnapshot,
};
use stillwind_source::{FileId, SourceError, SourceFile};

#[derive(Debug, thiserror::Error)]
pub(super) enum FileLoadError {
    #[error("{0}")]
    Source(#[from] SourceError),
    #[error("cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot resolve {} imported by {}: {source}", .request.specifier, .request.importer.0)]
    Resolve {
        request: CssFileRequest,
        #[source]
        source: Box<ResolveError>,
    },
    #[error("{0}")]
    Load(#[from] DesignSystemLoadError),
}

pub(super) fn read_source(path: &str, id: u32) -> Result<SourceFile, FileLoadError> {
    let text = fs::read_to_string(path).map_err(|error| FileLoadError::Read {
        path: path.into(),
        source: error,
    })?;
    Ok(SourceFile::new(FileId(id), path, text)?)
}

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
