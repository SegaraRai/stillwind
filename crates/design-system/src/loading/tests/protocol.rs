//! Checks host requests, batches, traversal order, and graph edges.

use super::*;

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

/// An import that closes a cycle is skipped, while repeated names with different
/// content and invalid CSS are distinct protocol errors.
#[test]
fn loader_skips_cycles_and_distinguishes_conflicting_identity_and_invalid_css() {
    let mut cycle =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let css = "@import 'self'; .a{}";
    assert!(matches!(
        cycle.provide(0, CssSourceName::from("root"), None, None, css.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) =
        cycle.provide(1, CssSourceName::from("root"), None, None, css.into())
    else {
        panic!("expected a snapshot")
    };
    assert!(matches!(
        snapshot.sources().skipped.as_slice(),
        [SkippedCss { reason: SkipReason::ImportCycle { specifier }, .. }] if specifier == "self"
    ));
    assert_eq!(snapshot.theme().custom_css(), " .a{}");
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

/// The import that closes a cycle is skipped where the traversal from the root first
/// returns to a source on its path, and the load waits for unrelated reads.
#[test]
fn cycles_are_skipped_after_every_file_arrives() {
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
    assert!(matches!(step, DesignSystemLoadStep::NeedFile(_)));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) =
        loader.provide(2, CssSourceName::from("slow"), None, None, "".into())
    else {
        panic!("expected a snapshot");
    };
    let sources = &snapshot.sources().sources;
    let [skipped] = snapshot.sources().skipped.as_slice() else {
        panic!("{:?}", snapshot.sources().skipped);
    };
    let b = sources
        .iter()
        .find(|source| source.name == CssSourceName::from("b"))
        .unwrap();
    assert_eq!(skipped.span.file, b.source.id());
    assert_eq!(b.source.slice(skipped.span).unwrap(), "@import 'a';");
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

fn complete(step: DesignSystemLoadStep) -> DesignSystemSnapshot {
    match step {
        DesignSystemLoadStep::Complete(Ok(snapshot)) => snapshot,
        other => panic!("expected snapshot: {other:?}"),
    }
}
