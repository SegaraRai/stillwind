use stillwind_core::{CompilerOptions, DesignSystemSnapshot};
use stillwind_core::{DesignSystemLoadError, DesignSystemLoadStep, DesignSystemLoader};
use stillwind_source::{FileId, SourceFile};

#[test]
fn loader_traverses_opaque_names_in_authored_order_and_reuses_sources() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
    let root = "not/a\\path:ROOT";
    let step = loader.provide(
        root.into(),
        "@import 'one'; @import 'alias'; @theme { --color-brand: red; }".into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected request")
    };
    assert_eq!(request.importer.0.as_ref(), root);
    assert_eq!(request.specifier, "one");
    let child = "@theme { --color-brand: blue; }";
    let step = loader.provide("Opaque CHILD".into(), child.into());
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected alias request")
    };
    assert_eq!(request.specifier, "alias");
    let DesignSystemLoadStep::Complete(Ok(snapshot)) =
        loader.provide("Opaque CHILD".into(), child.into())
    else {
        panic!("expected snapshot")
    };
    assert_eq!(snapshot.sources().sources.len(), 2);
    assert_eq!(snapshot.sources().edges.len(), 2);
    let flat = DesignSystemSnapshot::from_source(&SourceFile::new(FileId(1), "flat", "@theme { --color-brand: blue; } @theme { --color-brand: blue; } @theme { --color-brand: red; }"), CompilerOptions::default()).unwrap();
    assert_eq!(snapshot.render_stylesheet(""), flat.render_stylesheet(""));
    assert!(matches!(
        loader.provide("extra".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Protocol { .. }))
    ));
}

#[test]
fn loader_distinguishes_cycles_conflicting_identity_and_invalid_css() {
    let mut cycle = DesignSystemLoader::new(CompilerOptions::default());
    let css = "@import 'self';";
    assert!(matches!(
        cycle.provide("root".into(), css.into()),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Cycle { chain })) =
        cycle.provide("root".into(), css.into())
    else {
        panic!("expected cycle")
    };
    assert_eq!(chain[0].specifier, "self");
    let mut conflict = DesignSystemLoader::new(CompilerOptions::default());
    conflict.provide("root".into(), css.into());
    assert!(matches!(
        conflict.provide("root".into(), "".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Protocol { .. }))
    ));
    let mut invalid = DesignSystemLoader::new(CompilerOptions::default());
    assert!(matches!(
        invalid.provide("broken".into(), "@theme {".into()),
        DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::InvalidCss { .. }))
    ));
}

#[test]
fn loader_tokenizes_imports_and_ignores_comments_strings_and_nested_rules() {
    let mut loader = DesignSystemLoader::new(CompilerOptions::default());
    let step = loader.provide(
        "root".into(),
        "/* @import 'fake'; */ @import url(\"real.css\"); .x { content: \"@import 'fake';\"; }"
            .into(),
    );
    let DesignSystemLoadStep::NeedFile(request) = step else {
        panic!("expected real import")
    };
    assert_eq!(request.specifier, "real.css");
    assert!(matches!(
        loader.provide("child".into(), "".into()),
        DesignSystemLoadStep::Complete(Ok(_))
    ));
}

#[test]
fn snapshots_pin_revisions_and_fingerprint_content_without_source_spans() {
    let first = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: red; }"),
        CompilerOptions::default(),
    )
    .unwrap();
    let identical = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(87),
            "opaque:second",
            "@theme { --color-brand: red; }",
        ),
        CompilerOptions::default(),
    )
    .unwrap();
    let next = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "opaque:first", "@theme { --color-brand: blue; }"),
        CompilerOptions::default(),
    )
    .unwrap();
    let pinned = first.clone();
    assert_eq!(pinned.revision(), first.revision());
    assert_ne!(first.revision(), identical.revision());
    assert_eq!(first.fingerprints(), identical.fingerprints());
    assert_ne!(first.fingerprints().content, next.fingerprints().content);
    assert_eq!(first.fingerprints().config, next.fingerprints().config);
    assert_eq!(first.render_stylesheet(""), pinned.render_stylesheet(""));
}

#[test]
fn snapshot_validates_css_resolved_configuration() {
    let options = CompilerOptions {
        prefix: "bad".into(),
        ..CompilerOptions::default()
    };
    assert!(
        DesignSystemSnapshot::from_source(&SourceFile::new(FileId(0), "root", ""), options)
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
        ),
        CompilerOptions::default(),
    )
    .unwrap();
    assert!(snapshot.utilities().get("flex").is_some());
    assert_eq!(snapshot.utilities().get("custom").unwrap().len(), 2);
    assert!(snapshot.fingerprints().utilities.contains_key("flex"));
    assert!(snapshot.builtins().is_some());
    let unselected = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "root", "@import 'stillwind/theme.css';"),
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
        "root".into(),
        "@import 'child'; @theme { --color-root: red; }".into(),
    );
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
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
            "root".into(),
            "@import 'child' layer(components) supports(display: grid) screen and (width > 40rem);"
                .into()
        ),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let step = loader.provide("child".into(), ".card { display: grid; }".into());
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = step else {
        panic!("{step:?}")
    };
    let css = snapshot.render_stylesheet("");
    assert!(css.contains("@layer components"), "{css}");
    assert!(css.contains("@supports (display: grid)"), "{css}");
    assert!(css.contains("@media screen and (width > 40rem)"), "{css}");
}
