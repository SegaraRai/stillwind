//! Document-level stylesheet assembly through the design-system loader.
//!
//! These tests pin that rendered CSS follows the authored import order, keeps each import's
//! layer, supports, and media conditions, stays independent of the order in which the host
//! answers file requests, and renders identically from a cloned snapshot.

use stillwind_compiler::{CompilerOptions, Identifiers, StylesheetRenderer};
use stillwind_design_system::{
    CssFileInput, DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions,
    DesignSystemSnapshot,
};
use stillwind_source::{CssSourceName, FileId, SourceFile, SourceRegistry};

#[test]
fn renders_repeated_imports_in_authored_order() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let root = "not/a\\path:ROOT";
    let step = loader.provide(
        0,
        root.into(),
        None,
        None,
        "@import 'one'; @import 'alias'; @theme { --color-brand: red; }".into(),
    );
    let DesignSystemLoadStep::NeedFile(_) = step else {
        panic!("expected request")
    };
    let child = "@theme { --color-brand: blue; }";
    let step = loader.provide(1, "Opaque CHILD".into(), None, None, child.into());
    let DesignSystemLoadStep::NeedFile(_) = step else {
        panic!("expected alias request")
    };
    let DesignSystemLoadStep::Complete(Ok(snapshot)) =
        loader.provide(2, "Opaque CHILD".into(), None, None, child.into())
    else {
        panic!("expected snapshot")
    };
    let flat = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(1), None, None,
            "@theme { --color-brand: blue; } @theme { --color-brand: blue; } @theme { --color-brand: red; }"
        ).unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default()
    ).unwrap();
    assert_eq!(
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_stylesheet("")
        .unwrap()
        .stylesheet
        .css,
        StylesheetRenderer::new(&flat, &CompilerOptions::default(), &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
    );
}

#[test]
fn imported_styles_retain_layer_supports_and_media_conditions() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    assert!(matches!(
        loader.provide(
            0,
            "root".into(),
            None,
            None,
            "@import 'child' layer(components) supports(display: grid) screen and (width > 40rem);"
                .into()
        ),
        DesignSystemLoadStep::NeedFile(_)
    ));
    let step = loader.provide(
        1,
        "child".into(),
        None,
        None,
        ".card { display: grid; }".into(),
    );
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = step else {
        panic!("{step:?}")
    };
    let css = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::default(),
    )
    .render_stylesheet("")
    .unwrap()
    .stylesheet
    .css;
    assert!(css.contains("@layer components"), "{css}");
    assert!(css.contains("@supports (display: grid)"), "{css}");
    assert!(css.contains("@media screen and (width > 40rem)"), "{css}");
}

#[test]
fn package_rendering_resolves_theme_and_requires_explicit_imports() {
    fn load() -> DesignSystemSnapshot {
        use std::path::Path;

        let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/core/index.css")
            .canonicalize()
            .unwrap();
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        let mut step = loader.provide(
            0,
            entry.to_string_lossy().as_ref().into(),
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
                            let source = std::fs::read_to_string(&file).unwrap();
                            input(request.id, file.to_string_lossy().as_ref(), &source)
                        })
                        .collect::<Vec<_>>();
                    step = loader.provide_many(files);
                }
            }
        }
    }

    let first = load();
    assert!(
        StylesheetRenderer::new(&first, &CompilerOptions::default(), &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
            .contains("box-sizing")
    );
    assert!(
        !StylesheetRenderer::new(&first, &CompilerOptions::default(), &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
            .contains("--theme(")
    );
    let bare = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "").unwrap(),
        "test.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert!(
        !StylesheetRenderer::new(&bare, &CompilerOptions::default(), &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
            .contains("box-sizing")
    );
}

#[test]
fn rendering_is_independent_of_import_response_order() {
    let root = "@import 'a'; @import 'b'; .root { display: block; }";
    let a = "@theme { --color-brand: red; } .a { color: red; }";
    let b = "@import 'c'; @theme { --color-brand: blue; } .b { color: blue; }";
    let c = "@theme { --color-child: green; } .c { color: green; }";
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    loader.provide(0, "root".into(), None, None, root.into());
    loader.provide(2, "b".into(), None, None, b.into());
    loader.provide(3, "c".into(), None, None, c.into());
    let snapshot = complete(loader.provide(1, "a".into(), None, None, a.into()));

    let mut batched =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    batched.provide_many([input(0, "root", root)]);
    batched.provide_many([input(2, "b", b), input(1, "a", a)]);
    let batch_snapshot = complete(batched.provide_many([input(3, "c", c)]));
    let flat = DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(1),
            None,
            None,
            format!("{a} {c} @theme {{ --color-brand: blue; }} .b {{ color: blue; }} .root {{ display: block; }}")
        ).unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default()
    ).unwrap();
    assert_eq!(
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_stylesheet("")
        .unwrap()
        .stylesheet
        .css,
        StylesheetRenderer::new(&flat, &CompilerOptions::default(), &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
    );
    assert_eq!(
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_stylesheet("")
        .unwrap()
        .stylesheet
        .css,
        StylesheetRenderer::new(
            &batch_snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_stylesheet("")
        .unwrap()
        .stylesheet
        .css
    );
}

#[test]
fn renders_each_diamond_import_occurrence() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    loader.provide(
        0,
        "root".into(),
        None,
        None,
        "@import 'a'; @import 'alias';".into(),
    );
    loader.provide_many([
        input(2, "shared", "@import 'leaf'; .shared { color: red; }"),
        input(1, "shared", "@import 'leaf'; .shared { color: red; }"),
    ]);
    let snapshot = complete(loader.provide(
        3,
        "leaf".into(),
        None,
        None,
        ".leaf { color: blue; }".into(),
    ));
    let css = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::default(),
    )
    .render_stylesheet("")
    .unwrap()
    .stylesheet
    .css;
    assert_eq!(css.matches(".leaf").count(), 2);
    assert_eq!(css.matches(".shared").count(), 2);
}

#[test]
fn cloned_snapshots_render_identically() {
    let snapshot = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "@theme { --color-brand: red; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let pinned = snapshot.clone();
    let options = CompilerOptions::default();
    assert_eq!(
        StylesheetRenderer::new(&snapshot, &options, &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css,
        StylesheetRenderer::new(&pinned, &options, &Identifiers::default())
            .render_stylesheet("")
            .unwrap()
            .stylesheet
            .css
    );
}

fn input(id: u32, name: &str, content: &str) -> CssFileInput {
    CssFileInput {
        id,
        name: name.into(),
        uri: None,
        source_map_source: None,
        content: content.into(),
        lint: false,
        unresolved: false,
    }
}

fn complete(step: DesignSystemLoadStep) -> DesignSystemSnapshot {
    match step {
        DesignSystemLoadStep::Complete(Ok(snapshot)) => snapshot,
        other => panic!("expected snapshot: {other:?}"),
    }
}
