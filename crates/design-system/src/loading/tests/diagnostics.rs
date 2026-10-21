//! Checks source failures and authored resource provenance.

use std::error::Error;

use super::*;

#[test]
fn parse_failures_keep_source_span_and_typed_cause() {
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

/// Skipped statements are recorded in their own sources, including statements that only
/// the expansion reveals, and never expose the synthetic file IDs that expansion uses
/// internally.
#[test]
fn skipped_statements_keep_their_authored_sources() {
    let root = "@import 123; @import 'child.css' screen;";
    let child = "@theme { --color-red: red; }";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    assert!(matches!(
        loader.provide(0, CssSourceName::from("root.css"), None, None, root.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        1,
        CssSourceName::from("child.css"),
        None,
        None,
        child.into(),
    ) else {
        panic!("expected a snapshot")
    };
    let sources = &snapshot.sources().sources;
    let [import, condition] = snapshot.sources().skipped.as_slice() else {
        panic!("{:?}", snapshot.sources().skipped);
    };
    assert_eq!(import.span.file, sources[0].source.id());
    assert_eq!(
        sources[0].source.slice(import.span).unwrap(),
        "@import 123;"
    );
    assert!(matches!(import.reason, SkipReason::ImportSpecifier { .. }));
    assert_eq!(condition.span.file, sources[1].source.id());
    assert_eq!(sources[1].source.slice(condition.span).unwrap(), "@theme");
    assert!(matches!(
        condition.reason,
        SkipReason::ConditionalDefinition { .. }
    ));
    // The definition still applies, without the condition.
    assert!(snapshot.theme().token("color", "red").is_some());
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
