//! Loader and snapshot behaviour through the public protocol.
//!
//! Pins that response order and batch shape never change the resulting snapshot, that
//! opaque names identify sources, that cycles and conflicting content are reported with
//! the offending chain, and that token spans keep pointing into the buffer the host
//! supplied. A package import stays a host request, and parse failures keep a typed
//! `#[source]` cause and a span in the supplied buffer.

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{CssSourceName, FileId, SourceFile, SourceRegistry, SourceUri};
use stillwind_syntax::parse_candidate;

use super::*;
use crate::{DesignSystemOptions, DesignSystemSnapshot, resolve_variant};

#[test]
fn tailwind_import_is_a_host_request_not_a_stillwind_alias() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let DesignSystemLoadStep::NeedFile(requests) = loader.provide(
        0,
        CssSourceName::from("root.css"),
        None,
        None,
        "@import 'tailwindcss';".into(),
    ) else {
        panic!("expected a real import request")
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].specifier, "tailwindcss");
}

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

#[test]
fn snapshot_reports_unresolved_requests_and_invalid_options_without_flattening() {
    use crate::DesignSystemError;
    let source = SourceFile::new(FileId(1), None, None, "@import 'child';").unwrap();
    let error = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, DesignSystemError::UnresolvedImports { requests } if requests.len() == 1 && requests[0].id == 1 && requests[0].specifier == "child")
    );
    let source = SourceFile::new(FileId(1), None, None, "").unwrap();
    let options = DesignSystemOptions {
        prefix: "bad".into(),
        ..DesignSystemOptions::default()
    };
    assert!(matches!(
        DesignSystemSnapshot::from_source(&source, CssSourceName::from("test.css"), options),
        Err(DesignSystemError::Load(
            DesignSystemLoadError::InvalidOptions(CompileDiagnostic::InvalidPropertyPrefix {
                option,
                ..
            })
        )) if option == "prefix"
    ));
}

#[test]
fn imported_variant_blocks_obey_later_shorthand_overrides() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let step = loader.provide(
        0,
        CssSourceName::from("root.css"),
        None,
        None,
        "@import 'child.css'; @custom-variant state (&:focus);".into(),
    );
    assert!(matches!(step, DesignSystemLoadStep::NeedFile(_)));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        1,
        CssSourceName::from("child.css"),
        None,
        None,
        "@variant state { &:hover { @slot; } }".into(),
    ) else {
        panic!("expected loaded snapshot");
    };
    let candidate = parse_candidate("state:x").unwrap();
    let variant = resolve_variant(&candidate.variants[0], snapshot.theme()).unwrap();
    assert!(format!("{variant:?}").contains(":focus"));
    assert!(!format!("{variant:?}").contains(":hover"));
}

/// Cycles, repeated names with different content, and invalid CSS are distinct
/// protocol errors rather than one generic failure.
#[test]
fn loader_distinguishes_cycles_conflicting_identity_and_invalid_css() {
    let mut cycle =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let css = "@import 'self';";
    assert!(matches!(
        cycle.provide(0, CssSourceName::from("root"), None, None, css.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Cycle { chain })) =
        cycle.provide(1, CssSourceName::from("root"), None, None, css.into())
    else {
        panic!("expected cycle")
    };
    assert_eq!(chain[0].specifier, "self");
    let mut conflict =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    conflict.provide(0, CssSourceName::from("root"), None, None, css.into());
    assert!(matches!(
        conflict.provide(1, CssSourceName::from("root"), None, None, "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::ConflictingSource { .. }))
    ));
    let mut changed_uri =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    changed_uri.provide(
        0,
        CssSourceName::from("root"),
        Some(SourceUri::parse("untitled:first").unwrap()),
        None,
        css.into(),
    );
    assert!(matches!(
        changed_uri.provide(
            1,
            CssSourceName::from("root"),
            Some(SourceUri::parse("untitled:second").unwrap()),
            None,
            css.into()
        ),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::ConflictingSource { .. }))
    ));
    let mut changed_map_source =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let uri = SourceUri::parse("untitled:root").unwrap();
    changed_map_source.provide(
        0,
        CssSourceName::from("root"),
        Some(uri.clone()),
        Some("first.css".into()),
        css.into(),
    );
    assert!(matches!(
        changed_map_source.provide(
            1,
            CssSourceName::from("root"),
            Some(uri),
            Some("second.css".into()),
            css.into()
        ),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::ConflictingSource { .. }))
    ));
    let mut invalid =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    assert!(matches!(
        invalid.provide(
            0,
            CssSourceName::from("broken"),
            None,
            None,
            "@theme {".into()
        ),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Compilation(_)))
    ));
}

#[test]
fn snapshot_validates_css_resolved_configuration() {
    let options = DesignSystemOptions {
        prefix: "bad".into(),
        ..DesignSystemOptions::default()
    };
    assert!(
        DesignSystemSnapshot::from_source(
            &SourceFile::new(FileId(0), None, None, "").unwrap(),
            CssSourceName::from("test.css"),
            options
        )
        .is_err()
    );
}

/// Importing the package selects built-ins; authoring a theme alone does not.
#[test]
fn resolved_registries_include_selected_builtins_and_authored_overloads() {
    let snapshot = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(1), None, None,
            "@stillwind-utility flex { display: flex; } @utility custom { color: red; } @utility custom { background: blue; }",
        )
        .unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(snapshot.utilities().get("flex").is_some());
    assert_eq!(snapshot.utilities().get("custom").unwrap().len(), 2);
    assert!(snapshot.fingerprints().utilities.contains_key("flex"));
    assert!(snapshot.builtins().is_some());
    let unselected = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "@theme { --color-brand:red; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(unselected.builtins().unwrap().utilities.is_empty());
    assert!(unselected.utilities().get("flex").is_none());
}

/// Token spans point into the exact buffer the host supplied for that file, not into a
/// concatenated or re-generated copy.
#[test]
fn imported_token_spans_refer_to_the_original_supplied_buffer() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    loader.provide(
        0,
        CssSourceName::from("root"),
        None,
        None,
        "@import 'child'; @theme { --color-root: red; }".into(),
    );
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        1,
        CssSourceName::from("child id"),
        None,
        None,
        "/* 日本語 */ @theme { --color-child: blue; }".into(),
    ) else {
        panic!("expected snapshot")
    };
    let token = &snapshot.tokens().get("--color-child").unwrap()[0];
    let child = snapshot
        .sources()
        .sources
        .iter()
        .find(|source| source.name.as_str() == "child id")
        .unwrap();
    assert_eq!(
        child.source.slice(token.name.span).unwrap(),
        "--color-child"
    );
    assert_eq!(child.source.slice(token.value.span).unwrap(), "blue");
    assert!(!snapshot.sources().mappings.is_empty());
}

/// A cycle is reported as soon as the offending file arrives, even while other files
/// are still outstanding.
#[test]
fn cycles_are_reported_while_other_files_are_outstanding() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    loader.provide(
        0,
        CssSourceName::from("root"),
        None,
        None,
        "@import 'a'; @import 'slow';".into(),
    );
    loader.provide(
        1,
        CssSourceName::from("a"),
        None,
        None,
        "@import 'b';".into(),
    );
    loader.provide(
        3,
        CssSourceName::from("b"),
        None,
        None,
        "@import 'a';".into(),
    );
    let step = loader.provide(
        4,
        CssSourceName::from("a"),
        None,
        None,
        "@import 'b';".into(),
    );
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Cycle { chain })) = step else {
        panic!("expected cycle");
    };
    assert_eq!(
        chain.iter().map(|request| request.id).collect::<Vec<_>>(),
        [1, 3, 4]
    );
}

/// Only the IDs the loader issued are accepted, and only once each.
#[test]
fn loader_validates_batches_against_previously_issued_ids() {
    for files in [vec![], vec![input(0, "a", ""), input(1, "b", "")]] {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        assert!(matches!(
            loader.provide_many(files),
            DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::InvalidInitialBatch { .. }))
        ));
    }
    for files in [
        vec![input(0, "a", "")],
        vec![input(99, "a", "")],
        vec![input(1, "a", ""), input(1, "a", "")],
        vec![input(1, "a", "@import 'new';"), input(3, "new", "")],
        vec![input(1, "same", ".a{}"), input(2, "same", ".b{}")],
    ] {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        loader.provide(
            0,
            CssSourceName::from("root"),
            None,
            None,
            "@import 'a'; @import 'b';".into(),
        );
        assert!(matches!(
            loader.provide_many(files),
            DesignSystemLoadStep::Complete(Err(
                DesignSystemLoadError::UnknownOrDuplicateRequest { .. }
                    | DesignSystemLoadError::ConflictingSource { .. }
            ))
        ));
    }
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    // Root ID 0 is conventional, not mandatory.
    loader.provide(
        42,
        CssSourceName::from("root"),
        None,
        None,
        "@import 'a'; @import 'b';".into(),
    );
    assert_eq!(requests(loader.provide_many([])).len(), 2);
    loader.provide(1, CssSourceName::from("a"), None, None, "".into());
    assert!(matches!(
        loader.provide(1, CssSourceName::from("a"), None, None, "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::UnknownOrDuplicateRequest {
            id: 1
        }))
    ));
}

/// A revision is pinned per snapshot; fingerprints hash content without spans, so the
/// same bytes under a different file ID keep the same content fingerprint.
#[test]
fn snapshots_pin_revisions_and_fingerprint_content_without_source_spans() {
    let first = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "@theme { --color-brand: red; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let identical = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(87), None, None, "@theme { --color-brand: red; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let next = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "@theme { --color-brand: blue; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let pinned = first.clone();
    assert_eq!(pinned.revision(), first.revision());
    assert_ne!(first.revision(), identical.revision());
    assert_eq!(first.fingerprints(), identical.fingerprints());
    assert_ne!(first.fingerprints().content, next.fingerprints().content);
    assert_eq!(first.fingerprints().config, next.fingerprints().config);
}

/// Names are opaque: traversal order is the authored order, and re-supplying the same
/// name with the same content reuses the loaded source.
#[test]
fn loader_traverses_opaque_names_in_authored_order_and_reuses_sources() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let root = r"not/a\path:ROOT";
    let step = loader.provide(
        0,
        CssSourceName::from(root),
        None,
        None,
        "@import 'one'; @import 'alias'; @theme { --color-brand: red; }".into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected request")
    };
    assert_eq!(request[0].importer.as_str(), root);
    assert_eq!(request[0].specifier, "one");
    let child = "@theme { --color-brand: blue; }";
    let step = loader.provide(
        1,
        CssSourceName::from("Opaque CHILD"),
        None,
        None,
        child.into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected alias request")
    };
    assert_eq!(request[0].specifier, "alias");
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        2,
        CssSourceName::from("Opaque CHILD"),
        None,
        None,
        child.into(),
    ) else {
        panic!("expected snapshot")
    };
    assert_eq!(snapshot.sources().sources.len(), 2);
    assert_eq!(snapshot.sources().edges.len(), 2);
    assert!(matches!(
        loader.provide(0, CssSourceName::from("extra"), None, None, "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete))
    ));
}

/// Partial and out-of-order responses produce the same snapshot as an ordered,
/// one-file-at-a-time exchange.
#[test]
fn partial_out_of_order_responses_preserve_fingerprints_and_spans() {
    let root = "@import 'a'; @import 'b'; .root { display: block; }";
    let a = "@theme { --color-brand: red; } .a { color: red; }";
    let b = "@import 'c'; @theme { --color-brand: blue; } .b { color: blue; }";
    let c = "@theme { --color-child: green; } .c { color: green; }";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let initial = requests(loader.provide(0, CssSourceName::from("root"), None, None, root.into()));
    assert_eq!(
        initial
            .iter()
            .map(|r| (r.id, r.specifier.as_str()))
            .collect::<Vec<_>>(),
        [(1, "a"), (2, "b")]
    );
    let pending = requests(loader.provide(2, CssSourceName::from("b"), None, None, b.into()));
    assert_eq!(pending.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(pending[1].importer.as_str(), "b");
    assert_eq!(pending[1].specifier, "c");
    assert_eq!(
        requests(loader.provide(3, "c".into(), None, None, c.into()))[0].id,
        1
    );
    let snapshot = complete(loader.provide(1, CssSourceName::from("a"), None, None, a.into()));

    let mut batched =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    batched.provide_many([input(0, "root", root)]);
    assert_eq!(
        requests(batched.provide_many([input(2, "b", b), input(1, "a", a)]))[0].id,
        3
    );
    let batch_snapshot = complete(batched.provide_many([input(3, "c", c)]));
    assert_eq!(snapshot.fingerprints(), batch_snapshot.fingerprints());
    let token = &snapshot.tokens().get("--color-child").unwrap()[0];
    let source = snapshot
        .sources()
        .sources
        .iter()
        .find(|source| source.name.as_str() == "c")
        .unwrap();
    assert_eq!(source.source.slice(token.value.span).unwrap(), "green");
}

/// Two importers naming the same pending file share one request, and both edges stay.
#[test]
fn diamond_sources_share_pending_requests_and_preserve_graph_edges() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    loader.provide(
        0,
        CssSourceName::from("root"),
        None,
        None,
        "@import 'a'; @import 'alias';".into(),
    );
    let pending = requests(loader.provide_many([
        input(2, "shared", "@import 'leaf'; .shared { color: red; }"),
        input(1, "shared", "@import 'leaf'; .shared { color: red; }"),
    ]));
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, 3);
    let snapshot = complete(loader.provide(
        3,
        CssSourceName::from("leaf"),
        None,
        None,
        ".leaf { color: blue; }".into(),
    ));
    assert_eq!(snapshot.sources().sources.len(), 3);
    assert_eq!(snapshot.sources().edges.len(), 3);
}

/// Builds a response the way a host would after reading a file. The loader never
/// inspects the name, so a test may use any string it likes.
fn input(id: u32, name: &str, content: &str) -> CssFileInput {
    CssFileInput {
        id,
        name: CssSourceName::from(name),
        uri: None,
        source_map_source: None,
        content: content.into(),
    }
}

fn requests(step: DesignSystemLoadStep) -> Vec<CssFileRequest> {
    match step {
        DesignSystemLoadStep::NeedFile(requests) => requests,
        other @ DesignSystemLoadStep::Complete(_) => {
            panic!("expected outstanding requests: {other:?}")
        }
    }
}

fn complete(step: DesignSystemLoadStep) -> DesignSystemSnapshot {
    match step {
        DesignSystemLoadStep::Complete(Ok(snapshot)) => snapshot,
        other => panic!("expected snapshot: {other:?}"),
    }
}

/// Updating one package file replaces the loaded definitions instead of merging with
/// them, and importing nothing supplies no implicit defaults.
#[test]
fn package_updates_replace_loaded_definitions_without_implicit_defaults() {
    fn load(display: &str) -> DesignSystemSnapshot {
        use std::path::Path;

        let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/core/index.css")
            .canonicalize()
            .unwrap();
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        let mut step = loader.provide(
            0,
            CssSourceName::from(entry.to_string_lossy().as_ref()),
            None,
            None,
            std::fs::read_to_string(&entry).unwrap().into(),
        );
        loop {
            match step {
                DesignSystemLoadStep::Complete(result) => return result.unwrap(),
                DesignSystemLoadStep::NeedFile(requests) => {
                    let files = requests
                        .into_iter()
                        .map(|request| {
                            let file = Path::new(request.importer.as_str())
                                .parent()
                                .unwrap()
                                .join(&request.specifier)
                                .canonicalize()
                                .unwrap();
                            let source = std::fs::read_to_string(&file)
                                .unwrap()
                                .replace("display: flex;", &format!("display: {display};"));
                            input(request.id, file.to_string_lossy().as_ref(), &source)
                        })
                        .collect::<Vec<_>>();
                    step = loader.provide_many(files);
                }
            }
        }
    }

    let first = load("flex");
    let changed = load("grid");
    assert!(first.sources().sources.len() > 20);
    assert!(
        first.utilities().get("container").is_some(),
        "manifest documentation must not consume the first utility definition"
    );
    assert!(
        first.utilities().get("flex").unwrap()[0]
            .body
            .contains("display: flex;")
    );
    assert!(
        changed.utilities().get("flex").unwrap()[0]
            .body
            .contains("display: grid;")
    );
    assert_ne!(first.fingerprints().content, changed.fingerprints().content);
    let bare = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(bare.utilities().get("flex").is_none());
    assert!(bare.tokens().get("--spacing").is_none());
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let DesignSystemLoadStep::NeedFile(requests) = loader.provide(
        0,
        CssSourceName::from("entry"),
        None,
        None,
        "@import 'stillwind' layer(app) supports(display:grid) print;".into(),
    ) else {
        panic!("package must be resolved by the host")
    };
    assert_eq!(requests[0].specifier, "stillwind");
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
