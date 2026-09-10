use super::*;

#[test]
fn generated_webref_snapshot_covers_every_internal_complex_value_domain() {
    for property in [
        "animation",
        "columns",
        "contain",
        "cursor",
        "flex",
        "font-feature-settings",
        "font-stretch",
        "grid-auto-columns",
        "grid-column",
        "grid-template-columns",
        "list-style-image",
        "list-style-type",
        "tab-size",
        "transition-property",
        "vertical-align",
        "will-change",
        "zoom",
    ] {
        let definition =
            webref_property(property).unwrap_or_else(|| panic!("Webref must define `{property}`"));
        assert!(!definition.syntax.is_empty(), "{property}");
        assert!(definition.href.starts_with("https://"), "{property}");
    }
}
