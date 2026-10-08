//! Checks source failures and authored resource provenance.

use super::*;

#[test]
fn parse_failures_keep_source_span_and_typed_cause() {
    use std::error::Error;
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let DesignSystemLoadStep::Complete(Err(error)) = loader.provide(
        0,
        CssSourceName::from("opaque"),
        None,
        None,
        "@import 123;".into(),
    ) else {
        panic!("expected import error")
    };
    assert!(matches!(
        &error,
        DesignSystemLoadError::Compilation(CompileError {
            diagnostic: CompileDiagnostic::InvalidImportSpecifier { offset: 0, .. },
            ..
        })
    ));
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<CompileDiagnostic>()
            .is_some()
    );
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let DesignSystemLoadStep::Complete(Err(error)) = loader.provide(
        0,
        CssSourceName::from("opaque"),
        None,
        None,
        "@theme {".into(),
    ) else {
        panic!("expected theme error")
    };
    assert!(matches!(
        &error,
        DesignSystemLoadError::Compilation(CompileError { .. })
    ));
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<CompileDiagnostic>()
            .is_some()
    );
    // The single-buffer entry reports the same identity for the same failure.
    let source = SourceFile::new(FileId(77), None, None, "@theme {").unwrap();
    let error = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap_err()
    .into_compile_error(source.full_span());
    assert_eq!(error.labels[0].span, source.full_span());
    assert!(matches!(
        error.diagnostic,
        CompileDiagnostic::UnterminatedCssBlock { .. }
    ));
}

/// Errors raised while expanding imported CSS keep the importer's source identity and
/// never expose the synthetic file IDs expansion uses internally.
#[test]
fn expanded_css_errors_do_not_expose_synthetic_file_ids() {
    let root = "@import 'child.css' screen;";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    assert!(matches!(
        loader.provide(0, CssSourceName::from("root.css"), None, None, root.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Compilation(error))) = loader
        .provide(
            1,
            CssSourceName::from("child.css"),
            None,
            None,
            "@theme { --color-red: red; }".into(),
        )
    else {
        panic!("expected conditional definition failure")
    };
    assert!(matches!(
        error.diagnostic,
        CompileDiagnostic::ConditionalDefinition { .. }
    ));
    assert_eq!(
        error.labels[0].span,
        loader.root_source().unwrap().full_span()
    );
}

/// Repeated identical resource specifiers in one owner are one placeholder, so a host
/// resolves and rewrites the URL once.
#[test]
fn repeated_resource_specifiers_share_one_placeholder() {
    let snapshot = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(1), None, None,
            "@utility image { background-image:url('./same.svg'); mask-image:url('./same.svg'); } @stillwind-slot utilities;",
        )
        .unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let resources = &snapshot.sources().resources;
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].placeholder, resources[1].placeholder);
}
