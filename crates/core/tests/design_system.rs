use stillwind_core::{CompilerOptions, DesignSystemSnapshot};
use stillwind_source::{FileId, SourceFile};

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
