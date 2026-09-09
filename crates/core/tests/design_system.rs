use stillwind_core::{
    CompilerOptions, CssFileInput, CssFileRequest, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_source::{FileId, SourceFile};

#[test]
fn loader_traverses_opaque_names_in_authored_order_and_reuses_sources() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
    let flat = DesignSystemSnapshot::from_source(&SourceFile::new(FileId(1), "flat", "@theme { --color-brand: blue; } @theme { --color-brand: blue; } @theme { --color-brand: red; }").unwrap(), CompilerOptions::default()).unwrap();
    assert_eq!(
        snapshot.render_stylesheet("").unwrap(),
        flat.render_stylesheet("").unwrap()
    );
    assert!(matches!(
        loader.provide(0, "extra".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete))
    ));
}

#[test]
fn loader_distinguishes_cycles_conflicting_identity_and_invalid_css() {
    let mut cycle = DesignSystemLoader::new(CompilerOptions::default());
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
    let mut conflict = DesignSystemLoader::new(CompilerOptions::default());
    conflict.provide(0, "root".into(), css.into());
    assert!(matches!(
        conflict.provide(1, "root".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::ConflictingContent { .. }))
    ));
    let mut invalid = DesignSystemLoader::new(CompilerOptions::default());
    assert!(matches!(
        invalid.provide(0, "broken".into(), "@theme {".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::InvalidCss { .. }))
    ));
}

#[test]
fn loader_tokenizes_imports_and_ignores_comments_strings_and_nested_rules() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
fn snapshots_pin_revisions_and_fingerprint_content_without_source_spans() {
    let first = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: red; }").unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    let identical = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(87),
            "opaque:second",
            "@theme { --color-brand: red; }",
        )
        .unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    let next = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: blue; }").unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    let pinned = first.clone();
    assert_eq!(pinned.revision(), first.revision());
    assert_ne!(first.revision(), identical.revision());
    assert_eq!(first.fingerprints(), identical.fingerprints());
    assert_ne!(first.fingerprints().content, next.fingerprints().content);
    assert_eq!(first.fingerprints().config, next.fingerprints().config);
    assert_eq!(
        first.render_stylesheet("").unwrap(),
        pinned.render_stylesheet("").unwrap()
    );
}

#[test]
fn snapshot_validates_css_resolved_configuration() {
    let options = CompilerOptions {
        prefix: "bad".into(),
        ..CompilerOptions::default()
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
            "@utility custom { color: red; } @utility custom { background: blue; }",
        )
        .unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    assert!(snapshot.utilities().get("flex").is_some());
    assert_eq!(snapshot.utilities().get("custom").unwrap().len(), 2);
    assert!(snapshot.fingerprints().utilities.contains_key("flex"));
    assert!(snapshot.builtins().is_some());
    let unselected = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "root", "@import 'stillwind/theme.css';").unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    assert!(unselected.builtins().is_none());
    assert!(unselected.utilities().get("flex").is_none());
}

#[test]
fn imported_token_spans_refer_to_the_original_supplied_buffer() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
fn imported_styles_retain_layer_supports_and_media_conditions() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
    assert!(matches!(
        loader.provide(
            0,
            "root".into(),
            "@import 'child' layer(components) supports(display: grid) screen and (width > 40rem);"
                .into()
        ),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let step = loader.provide(1, "child".into(), ".card { display: grid; }".into());
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = step else {
        panic!("{step:?}")
    };
    let css = snapshot.render_stylesheet("").unwrap();
    assert!(css.contains("@layer components"), "{css}");
    assert!(css.contains("@supports (display: grid)"), "{css}");
    assert!(css.contains("@media screen and (width > 40rem)"), "{css}");
}

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
fn partial_out_of_order_responses_discover_imports_without_changing_css_or_spans() {
    let root = "@import 'a'; @import 'b'; .root { display: block; }";
    let a = "@theme { --color-brand: red; } .a { color: red; }";
    let b = "@import 'c'; @theme { --color-brand: blue; } .b { color: blue; }";
    let c = "@theme { --color-child: green; } .c { color: green; }";
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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

    let mut batched = DesignSystemLoader::new(CompilerOptions::default());
    batched.provide_many([input(0, "root", root)]);
    assert_eq!(
        requests(batched.provide_many([input(2, "b", b), input(1, "a", a)]))[0].id,
        3
    );
    let batch_snapshot = complete(batched.provide_many([input(3, "c", c)]));
    let flat = DesignSystemSnapshot::from_source(&SourceFile::new(FileId(1), "flat", format!("{a} {c} @theme {{ --color-brand: blue; }} .b {{ color: blue; }} .root {{ display: block; }}")).unwrap(), CompilerOptions::default()).unwrap();
    assert_eq!(
        snapshot.render_stylesheet("").unwrap(),
        flat.render_stylesheet("").unwrap()
    );
    assert_eq!(
        snapshot.render_stylesheet("").unwrap(),
        batch_snapshot.render_stylesheet("").unwrap()
    );
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
fn diamond_sources_reuse_unfinished_nodes_and_keep_each_import_occurrence() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
    let css = snapshot.render_stylesheet("").unwrap();
    assert_eq!(css.matches(".leaf").count(), 2);
    assert_eq!(css.matches(".shared").count(), 2);
}

#[test]
fn cycles_are_reported_while_other_files_are_outstanding() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
        let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
        let mut loader = DesignSystemLoader::new(CompilerOptions::default());
        loader.provide(0, "root".into(), "@import 'a'; @import 'b';".into());
        assert!(matches!(
            loader.provide_many(files),
            DesignSystemLoadStep::Complete(Err(
                DesignSystemLoadError::UnknownOrDuplicateRequest { .. }
                    | DesignSystemLoadError::ConflictingContent { .. }
            ))
        ));
    }
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
