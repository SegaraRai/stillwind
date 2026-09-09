use std::{
    fs,
    path::{Path, PathBuf},
};
use stillwind_core::{
    CompilerOptions, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_source::{FileId, SourceFile};
pub(super) fn read_source(path: &str, id: u32) -> Result<SourceFile, FileLoadError> {
    let text = fs::read_to_string(path).map_err(|error| FileLoadError::Read {
        path: path.into(),
        source: error,
    })?;
    Ok(SourceFile::new(FileId(id), path, text))
}

pub(super) fn load_design_system(
    theme: &SourceFile,
    options: CompilerOptions,
) -> Result<DesignSystemSnapshot, FileLoadError> {
    let mut loader = DesignSystemLoader::new(options);
    let mut step = loader.provide(0, theme.path().into(), theme.text().into());
    loop {
        match step {
            DesignSystemLoadStep::Complete(result) => {
                return result.map_err(FileLoadError::Load);
            }
            DesignSystemLoadStep::NeedFile(requests) => {
                let request = &requests[0];
                let path = Path::new(request.importer.0.as_ref())
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&request.specifier);
                let path = fs::canonicalize(&path).map_err(|error| FileLoadError::Resolve {
                    request: request.clone(),
                    source: error,
                })?;
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

#[derive(Debug, thiserror::Error)]
pub(super) enum FileLoadError {
    #[error("cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot resolve {} imported by {}: {source}", .request.specifier, .request.importer.0)]
    Resolve {
        request: stillwind_core::CssFileRequest,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Load(#[from] stillwind_core::DesignSystemLoadError),
}
