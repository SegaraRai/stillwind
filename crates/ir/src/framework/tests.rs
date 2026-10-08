//! Contracts shared by the framework specifications.

use super::*;

#[test]
fn frameworks_declare_their_materialized_style_format() {
    for spec in [&react::SPEC, &preact::SPEC, &solid::SPEC] {
        assert_eq!(
            spec.style_format,
            StyleFormat::Object,
            "{}",
            spec.macro_import
        );
    }
    assert_eq!(vue::SPEC.style_format, StyleFormat::Object);
    assert_eq!(svelte::SPEC.style_format, StyleFormat::String);
    assert_eq!(astro::SPEC.style_format, StyleFormat::String);
    for spec in FRAMEWORKS {
        assert!(spec.class_property == "className" || spec.class_property == "class");
        assert!(spec.runtime_import.ends_with("/runtime"));
    }
}
