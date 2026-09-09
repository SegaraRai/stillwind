use stillwind_source::{FileId, SourceFile};

use crate::DesignSystemSnapshot;

use super::*;

#[test]
fn parse_failures_keep_source_identity_and_typed_cause() {
    use std::error::Error;
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let DesignSystemLoadStep::Complete(Err(error)) =
        loader.provide(0, "opaque".into(), "@import 123;".into())
    else {
        panic!("expected import error")
    };
    assert!(
        matches!(&error, DesignSystemLoadError::InvalidImport { name, cause: CssImportError::Specifier { offset: 0, .. } } if name.0.as_ref() == "opaque")
    );
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<CssImportError>()
            .is_some()
    );
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let DesignSystemLoadStep::Complete(Err(error)) =
        loader.provide(0, "opaque".into(), "@theme {".into())
    else {
        panic!("expected theme error")
    };
    assert!(
        matches!(&error, DesignSystemLoadError::InvalidCss { name, .. } if name.0.as_ref() == "opaque")
    );
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<stillwind_css::ThemeError>()
            .is_some()
    );
}

#[test]
fn snapshot_reports_unresolved_requests_and_invalid_options_without_flattening() {
    use crate::{CompilerOptionsError, DesignSystemError};
    let source = SourceFile::new(FileId(1), "root", "@import 'child';");
    let error =
        DesignSystemSnapshot::from_source(&source, DesignSystemOptions::default()).unwrap_err();
    assert!(
        matches!(error, DesignSystemError::UnresolvedImports { requests } if requests.len() == 1 && requests[0].id == 1 && requests[0].specifier == "child")
    );
    let source = SourceFile::new(FileId(1), "root", "");
    let options = DesignSystemOptions {
        prefix: "bad".into(),
        ..DesignSystemOptions::default()
    };
    assert!(matches!(
        DesignSystemSnapshot::from_source(&source, options),
        Err(DesignSystemError::Load(
            DesignSystemLoadError::InvalidOptions(CompilerOptionsError::InvalidPrefix {
                option: "prefix",
                ..
            })
        ))
    ));
}
