//! Loader and snapshot integration through the public API.
//!
//! Pins the host-visible protocol: response order and batch shape never change the
//! resulting snapshot, opaque names identify sources, cycles and conflicting content
//! are reported with the offending chain, and token spans keep pointing into the buffer
//! the host supplied.

use stillwind_design_system::{
    CssFileInput, CssFileRequest, DesignSystemLoadError, DesignSystemLoadStep, DesignSystemLoader,
    DesignSystemOptions, DesignSystemSnapshot,
};
use stillwind_source::{FileId, SourceFile};

#[test]
fn loader_distinguishes_cycles_conflicting_identity_and_invalid_css() {
    let mut cycle = DesignSystemLoader::new(DesignSystemOptions::default());
    let css = "@import 'self';";
    assert!(matches!(
        cycle.provide(0, "root".into(), css.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Cycle { chain })) =
        cycle.provide(1, "root".into(), css.into())
    else {
        panic!("expected cycle")
    };
    assert_eq!(chain[0].specifier, "self");
    let mut conflict = DesignSystemLoader::new(DesignSystemOptions::default());
    conflict.provide(0, "root".into(), css.into());
    assert!(matches!(
        conflict.provide(1, "root".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::ConflictingContent { .. }))
    ));
    let mut invalid = DesignSystemLoader::new(DesignSystemOptions::default());
    assert!(matches!(
        invalid.provide(0, "broken".into(), "@theme {".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::InvalidCss { .. }))
    ));
}

#[test]
fn loader_tokenizes_imports_and_ignores_comments_strings_and_nested_rules() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let step = loader.provide(
        0,
        "root".into(),
        "/* @import 'fake'; */ @import url(\"real.css\"); .x { content: \"@import 'fake';\"; }"
            .into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected real import")
    };
    assert_eq!(request[0].specifier, "real.css");
    assert!(matches!(
        loader.provide(1, "child".into(), "".into()),
        DesignSystemLoadStep::Complete(Ok(_))
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
            &SourceFile::new(FileId(0), "root", "").unwrap(),
            options
        )
        .is_err()
    );
}

#[test]
fn resolved_registries_include_selected_builtins_and_authored_overloads() {
    let snapshot = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(1),
            "root",
            "@stillwind-utility flex { display: flex; } @utility custom { color: red; } @utility custom { background: blue; }",
        )
        .unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(snapshot.utilities().get("flex").is_some());
    assert_eq!(snapshot.utilities().get("custom").unwrap().len(), 2);
    assert!(snapshot.fingerprints().utilities.contains_key("flex"));
    assert!(snapshot.builtins().is_some());
    let unselected = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "root", "@theme { --color-brand:red; }").unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(unselected.builtins().unwrap().utilities.is_empty());
    assert!(unselected.utilities().get("flex").is_none());
}

#[test]
fn imported_token_spans_refer_to_the_original_supplied_buffer() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.provide(
        0,
        "root".into(),
        "@import 'child'; @theme { --color-root: red; }".into(),
    );
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        1,
        "child id".into(),
        "/* 日本語 */ @theme { --color-child: blue; }".into(),
    ) else {
        panic!("expected snapshot")
    };
    let token = &snapshot.tokens().get("--color-child").unwrap()[0];
    let child = snapshot
        .sources()
        .sources
        .iter()
        .find(|source| source.name.0.as_ref() == "child id")
        .unwrap();
    assert_eq!(
        child.source.slice(token.name.span).unwrap(),
        "--color-child"
    );
    assert_eq!(child.source.slice(token.value.span).unwrap(), "blue");
    assert!(!snapshot.sources().mappings.is_empty());
}

#[test]
fn cycles_are_reported_while_other_files_are_outstanding() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.provide(0, "root".into(), "@import 'a'; @import 'slow';".into());
    loader.provide(1, "a".into(), "@import 'b';".into());
    loader.provide(3, "b".into(), "@import 'a';".into());
    let step = loader.provide(4, "a".into(), "@import 'b';".into());
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Cycle { chain })) = step else {
        panic!("expected cycle");
    };
    assert_eq!(
        chain.iter().map(|request| request.id).collect::<Vec<_>>(),
        [1, 3, 4]
    );
}

#[test]
fn loader_validates_batches_against_previously_issued_ids() {
    for files in [vec![], vec![input(0, "a", ""), input(1, "b", "")]] {
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
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
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        loader.provide(0, "root".into(), "@import 'a'; @import 'b';".into());
        assert!(matches!(
            loader.provide_many(files),
            DesignSystemLoadStep::Complete(Err(
                DesignSystemLoadError::UnknownOrDuplicateRequest { .. }
                    | DesignSystemLoadError::ConflictingContent { .. }
            ))
        ));
    }
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    // Root ID 0 is conventional, not mandatory.
    loader.provide(42, "root".into(), "@import 'a'; @import 'b';".into());
    assert_eq!(requests(loader.provide_many([])).len(), 2);
    loader.provide(1, "a".into(), "".into());
    assert!(matches!(
        loader.provide(1, "a".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::UnknownOrDuplicateRequest {
            id: 1
        }))
    ));
}

#[test]
fn snapshots_pin_revisions_and_fingerprint_content_without_source_spans() {
    let first = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: red; }").unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let identical = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(87),
            // Entry identity now namespaces anonymous output layers; only file IDs/spans vary.
            "opaque:first",
            "@theme { --color-brand: red; }",
        )
        .unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let next = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: blue; }").unwrap(),
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

#[test]
fn loader_traverses_opaque_names_in_authored_order_and_reuses_sources() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let root = "not/a\\path:ROOT";
    let step = loader.provide(
        0,
        root.into(),
        "@import 'one'; @import 'alias'; @theme { --color-brand: red; }".into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected request")
    };
    assert_eq!(request[0].importer.0.as_ref(), root);
    assert_eq!(request[0].specifier, "one");
    let child = "@theme { --color-brand: blue; }";
    let step = loader.provide(1, "Opaque CHILD".into(), child.into());
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected alias request")
    };
    assert_eq!(request[0].specifier, "alias");
    let DesignSystemLoadStep::Complete(Ok(snapshot)) =
        loader.provide(2, "Opaque CHILD".into(), child.into())
    else {
        panic!("expected snapshot")
    };
    assert_eq!(snapshot.sources().sources.len(), 2);
    assert_eq!(snapshot.sources().edges.len(), 2);
    assert!(matches!(
        loader.provide(0, "extra".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete))
    ));
}

#[test]
fn partial_out_of_order_responses_preserve_fingerprints_and_spans() {
    let root = "@import 'a'; @import 'b'; .root { display: block; }";
    let a = "@theme { --color-brand: red; } .a { color: red; }";
    let b = "@import 'c'; @theme { --color-brand: blue; } .b { color: blue; }";
    let c = "@theme { --color-child: green; } .c { color: green; }";
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let initial = requests(loader.provide(0, "root".into(), root.into()));
    assert_eq!(
        initial
            .iter()
            .map(|r| (r.id, r.specifier.as_str()))
            .collect::<Vec<_>>(),
        [(1, "a"), (2, "b")]
    );
    let pending = requests(loader.provide(2, "b".into(), b.into()));
    assert_eq!(pending.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(pending[1].importer.0.as_ref(), "b");
    assert_eq!(pending[1].specifier, "c");
    assert_eq!(requests(loader.provide(3, "c".into(), c.into()))[0].id, 1);
    let snapshot = complete(loader.provide(1, "a".into(), a.into()));

    let mut batched = DesignSystemLoader::new(DesignSystemOptions::default());
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
        .find(|source| source.name.0.as_ref() == "c")
        .unwrap();
    assert_eq!(source.source.slice(token.value.span).unwrap(), "green");
}

#[test]
fn diamond_sources_share_pending_requests_and_preserve_graph_edges() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    loader.provide(0, "root".into(), "@import 'a'; @import 'alias';".into());
    let pending = requests(loader.provide_many([
        input(2, "shared", "@import 'leaf'; .shared { color: red; }"),
        input(1, "shared", "@import 'leaf'; .shared { color: red; }"),
    ]));
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, 3);
    let snapshot = complete(loader.provide(3, "leaf".into(), ".leaf { color: blue; }".into()));
    assert_eq!(snapshot.sources().sources.len(), 3);
    assert_eq!(snapshot.sources().edges.len(), 3);
}

/// Builds a response the way a host would after reading a file. The loader never
/// inspects the name, so a test may use any string it likes.
fn input(id: u32, name: &str, content: &str) -> CssFileInput {
    CssFileInput {
        id,
        name: name.into(),
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

#[test]
fn package_updates_replace_loaded_definitions_without_implicit_defaults() {
    use std::path::Path;
    fn load(display: &str) -> DesignSystemSnapshot {
        let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/core/index.css")
            .canonicalize()
            .unwrap();
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        let mut step = loader.provide(
            0,
            entry.to_string_lossy().as_ref().into(),
            std::fs::read_to_string(&entry).unwrap().into(),
        );
        loop {
            match step {
                DesignSystemLoadStep::Complete(result) => return result.unwrap(),
                DesignSystemLoadStep::NeedFile(requests) => {
                    let files = requests
                        .into_iter()
                        .map(|request| {
                            let file = Path::new(request.importer.0.as_ref())
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
            .contains("display:flex;")
    );
    assert!(
        changed.utilities().get("flex").unwrap()[0]
            .body
            .contains("display:grid;")
    );
    assert_ne!(first.fingerprints().content, changed.fingerprints().content);
    let bare = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "empty", "").unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(bare.utilities().get("flex").is_none());
    assert!(bare.tokens().get("--spacing").is_none());
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let DesignSystemLoadStep::NeedFile(requests) = loader.provide(
        0,
        "entry".into(),
        "@import 'stillwind' layer(app) supports(display:grid) print;".into(),
    ) else {
        panic!("package must be resolved by the host")
    };
    assert_eq!(requests[0].specifier, "stillwind");
}
