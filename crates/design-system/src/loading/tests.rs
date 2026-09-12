//! Integration of import/theme errors with loader and snapshot error propagation.
//!
//! Pins that a package import stays a host request, that parse failures keep the opaque
//! source name and a typed `#[source]` cause, and that
//! [`DesignSystemSnapshot::from_source`] reports unresolved imports and invalid options
//! as distinct errors instead of flattening them.

use stillwind_source::{FileId, SourceFile};

use super::*;
use crate::{DesignSystemOptions, DesignSystemSnapshot};

#[test]
fn tailwind_import_is_a_host_request_not_a_stillwind_alias() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let DesignSystemLoadStep::NeedFile(requests) =
        loader.provide(0, "root.css".into(), "@import 'tailwindcss';".into())
    else {
        panic!("expected a real import request")
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].specifier, "tailwindcss");
}

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
            .downcast_ref::<crate::ThemeError>()
            .is_some()
    );
}

#[test]
fn snapshot_reports_unresolved_requests_and_invalid_options_without_flattening() {
    use crate::{DesignSystemError, DesignSystemOptionsError};
    let source = SourceFile::new(FileId(1), "root", "@import 'child';").unwrap();
    let error =
        DesignSystemSnapshot::from_source(&source, DesignSystemOptions::default()).unwrap_err();
    assert!(
        matches!(error, DesignSystemError::UnresolvedImports { requests } if requests.len() == 1 && requests[0].id == 1 && requests[0].specifier == "child")
    );
    let source = SourceFile::new(FileId(1), "root", "").unwrap();
    let options = DesignSystemOptions {
        prefix: "bad".into(),
        ..DesignSystemOptions::default()
    };
    assert!(matches!(
        DesignSystemSnapshot::from_source(&source, options),
        Err(DesignSystemError::Load(
            DesignSystemLoadError::InvalidOptions(DesignSystemOptionsError::InvalidPrefix {
                option: "prefix",
                ..
            })
        ))
    ));
}
