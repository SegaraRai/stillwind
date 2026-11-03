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

/// An unresolved import inside a style rule is recorded at its statement in the
/// importing source, and the rule keeps its other content.
#[test]
fn unresolved_nested_imports_keep_their_authored_sources() {
    let root = ".scope { color: red; @import 'missing.css'; }";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let request = requests(loader.provide(0, "root.css".into(), None, None, root.into()));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide_many([CssFileInput {
        unresolved: true,
        ..input(request[0].id, "missing.css", "")
    }]) else {
        panic!("expected a snapshot")
    };
    let sources = &snapshot.sources().sources;
    let [import] = snapshot.sources().skipped.as_slice() else {
        panic!("{:?}", snapshot.sources().skipped);
    };
    assert_eq!(
        sources[0].source.slice(import.span).unwrap(),
        "@import 'missing.css';"
    );
    assert!(matches!(import.reason, SkipReason::UnresolvedImport { .. }));
    assert_eq!(snapshot.theme().custom_css(), ".scope { color: red;  }");
}

/// A definition inside a style rule is ignored, and an import inside a style rule is
/// left out when the imported graph contains a statement that only the top level
/// accepts. Each record points at the statement that the author can change.
#[test]
fn top_level_statements_inside_rules_are_skipped_where_they_are_written() {
    let root = ".a { @utility direct { color: blue; } color: red; } .scope { @import 'a.css'; }";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let request = requests(loader.provide(0, "root.css".into(), None, None, root.into()));
    let request = requests(loader.provide(
        request[0].id,
        "a.css".into(),
        None,
        None,
        "@import 'b.css'; * { margin: 0; }".into(),
    ));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        request[0].id,
        "b.css".into(),
        None,
        None,
        "@utility imported { color: green; }".into(),
    ) else {
        panic!("expected a snapshot")
    };
    let root_source = &snapshot.sources().sources[0].source;
    let [definition, import] = snapshot.sources().skipped.as_slice() else {
        panic!("{:?}", snapshot.sources().skipped);
    };
    assert_eq!(
        root_source.slice(definition.span).unwrap(),
        "@utility direct { color: blue; }"
    );
    assert!(matches!(
        &definition.reason,
        SkipReason::DefinitionInStyleRule { directive } if directive == "utility"
    ));
    assert_eq!(root_source.slice(import.span).unwrap(), "@import 'a.css';");
    assert!(matches!(
        &import.reason,
        SkipReason::NestedImportDefinition { specifier, directive }
            if specifier == "a.css" && directive == "utility"
    ));
    assert_eq!(
        snapshot.theme().custom_css(),
        ".a {  color: red; } .scope {  }"
    );
    let positions = snapshot.theme().utility_positions();
    assert!(!positions.contains_key("direct") && !positions.contains_key("imported"));
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
