use super::*;

#[test]
fn request_id_exhaustion_is_terminal_and_never_emits_max_or_wraps() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.next_id = u32::MAX - 1;
    let DesignSystemLoadStep::NeedFile(requests) =
        loader.provide(0, "root".into(), "@import 'last';".into())
    else {
        panic!("expected last usable request ID");
    };
    assert_eq!(requests[0].id, u32::MAX - 1);
    assert!(matches!(
        loader.provide(u32::MAX - 1, "last".into(), "@import 'overflow';".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
    ));
    assert_eq!(loader.next_id, u32::MAX);
    assert!(matches!(
        loader.provide_many([]),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete))
    ));
}

#[test]
fn exhausted_batch_finishes_with_error_instead_of_returning_partial_requests() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.next_id = u32::MAX - 1;
    assert!(matches!(
        loader.provide(
            0,
            "root".into(),
            "@import 'last'; @import 'overflow';".into()
        ),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
    ));
    assert_eq!(loader.next_id, u32::MAX);
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
