//! Contracts shared by the framework specifications.

use super::*;

#[test]
fn frameworks_declare_their_materialized_style_format() {
    for spec in [
        &alpine::SPEC,
        &lit::SPEC,
        &preact::SPEC,
        &qwik::SPEC,
        &react::SPEC,
        &solid::SPEC,
        &vanilla::SPEC,
        &vue::SPEC,
    ] {
        assert_eq!(
            spec.output.spec().style_format,
            StyleFormat::Object,
            "{}",
            spec.macro_import
        );
    }
    assert_eq!(svelte::SPEC.output.spec().style_format, StyleFormat::String);
    assert_eq!(astro::SPEC.output.spec().style_format, StyleFormat::String);
    for spec in FRAMEWORKS {
        assert!(
            spec.output.spec().class_property == "className"
                || spec.output.spec().class_property == "class"
        );
        assert!(
            spec.output
                .spec()
                .runtime_import
                .starts_with("stillwind/runtime/")
        );
    }
}
