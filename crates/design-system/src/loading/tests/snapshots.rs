//! Checks pinned revisions, resolved configuration, and registry contents.

use lean_string::ToLeanString;

use stillwind_source::ReplaceText;

use super::*;
use crate::resolve_utility;

#[test]
fn snapshot_reports_unresolved_requests_and_invalid_options_without_flattening() {
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
    assert!(
        format_args!("{variant:?}")
            .to_lean_string()
            .contains(":focus")
    );
    assert!(
        !format_args!("{variant:?}")
            .to_lean_string()
            .contains(":hover")
    );
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
    assert!(Arc::ptr_eq(
        &snapshot.utilities().get("flex").unwrap()[0],
        &snapshot.stillwind_definitions().utilities()[0]
    ));
    let unselected = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), None, None, "@theme { --color-brand:red; }").unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert_eq!(
        unselected.stillwind_definitions().utilities(),
        [] as [Arc<CustomUtility>; 0]
    );
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
                                .join(request.specifier.as_str())
                                .canonicalize()
                                .unwrap();
                            let source = std::fs::read_to_string(&file).unwrap().replace_text(
                                "display: flex;",
                                &format_args!("display: {display};").to_lean_string(),
                            );
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

#[test]
fn plugin_definitions_precede_stillwind_definitions_and_the_stylesheet() {
    for root in [
        "@stillwind-utility size-full { width: 100%; } @utility probe { color: blue; } @plugin 'p';",
        "@stillwind-utility size-full { width: 100%; } @plugin 'p'; @utility probe { color: blue; }",
    ] {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        let request = requests(loader.provide(0, "root.css".into(), None, None, root.into()));
        let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
            request[0].id,
            "plugin".into(),
            None,
            None,
            "@utility icon { width: 1em; } @utility probe { color: red; }".into(),
        ) else {
            panic!("expected loaded snapshot");
        };
        let theme = snapshot.theme();
        let positions = theme.utility_positions();
        assert!(positions["icon"] < positions["size-full"], "{root}");
        assert!(positions["probe"] < positions["size-full"], "{root}");
        let probe = resolve_utility("probe", theme).unwrap().unwrap();
        let declaration = probe.declaration.replace(' ', "");
        assert!(
            declaration.find("color:red").unwrap() < declaration.find("color:blue").unwrap(),
            "{root}: {declaration}"
        );
    }
}

#[test]
fn stylesheet_variants_replace_plugin_variants_at_any_position() {
    for root in [
        "@custom-variant state (&:focus); @plugin 'p';",
        "@plugin 'p'; @custom-variant state (&:focus);",
    ] {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        let request = requests(loader.provide(0, "root.css".into(), None, None, root.into()));
        let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
            request[0].id,
            "plugin".into(),
            None,
            None,
            "@custom-variant state (&:hover);".into(),
        ) else {
            panic!("expected loaded snapshot");
        };
        let candidate = parse_candidate("state:x").unwrap();
        let variant = format_args!(
            "{:?}",
            resolve_variant(&candidate.variants[0], snapshot.theme()).unwrap()
        )
        .to_lean_string();
        assert!(variant.contains(":focus"), "{root}: {variant}");
        assert!(!variant.contains(":hover"), "{root}: {variant}");
    }
}

#[test]
fn trailing_plugin_answer_follows_the_root_text() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    assert!(loader.request_trailing_plugin("\0settings".into()));
    let request = requests(loader.provide(
        0,
        "root.css".into(),
        None,
        None,
        "@plugin 'p'; @utility probe { color: blue; }".into(),
    ));
    assert!(!loader.request_trailing_plugin("\0other".into()));
    let specifiers = request
        .iter()
        .map(|request| request.specifier.as_str())
        .collect::<Vec<_>>();
    assert_eq!(specifiers, ["p", "\0settings"]);
    assert!(matches!(
        &request[1].kind,
        CssDependencyKind::Plugin { options } if options.is_empty()
    ));
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide_many([
        input(request[0].id, "plugin", ""),
        input(
            request[1].id,
            "settings",
            "@custom-variant state (&:hover); @utility icon { width: 1em; }",
        ),
    ]) else {
        panic!("expected loaded snapshot");
    };
    let theme = snapshot.theme();
    assert!(theme.utility_positions()["icon"] < theme.utility_positions()["probe"]);
    let candidate = parse_candidate("state:x").unwrap();
    assert!(resolve_variant(&candidate.variants[0], theme).is_ok());
    assert!(
        snapshot
            .sources()
            .edges
            .iter()
            .any(|edge| edge.request.specifier == "\0settings" && edge.target == "settings".into())
    );
}

#[test]
fn config_statements_are_removed_from_the_expansion() {
    let mut loader =
        DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        0,
        "root.css".into(),
        None,
        None,
        ".a { color: red; } @config './tailwind.config.js'; .b { color: blue; }".into(),
    ) else {
        panic!("expected loaded snapshot");
    };
    let css = snapshot.theme().custom_css();
    assert!(!css.contains("@config"), "{css}");
    assert!(css.contains(".a") && css.contains(".b"), "{css}");
}
