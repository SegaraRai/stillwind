//! Checks generated identities and rejected names in host output.

use super::*;

/// Malformed sources are reported through the same schema a host reads.
#[test]
fn rejected_names_are_transported_beside_symbols() {
    let rejected = RejectedIdentifier {
        kind: IdentifierKind::RefMarker,
        hash: ("0123456789abcdef".to_owned()).into(),
        reason: ("no name for this declaration".to_owned()).into(),
        substituted: true,
    };
    let native: BindingRejectedIdentifier = rejected.into();
    let value: serde_json::Value = serde_json::to_value(&native).unwrap();
    assert_eq!(value["kind"], "ref-marker");
    assert_eq!(value["hash"], "0123456789abcdef");
    assert_eq!(value["substituted"], true);
}

#[test]
fn exposes_generated_symbols_for_build_wide_collision_checks() {
    let output = compile_source_input(
        &compile_input(
            r#"import { style } from "stillwind/react";
const menu = style.ref();
const props = style(menu.asAnchor());"#,
            "src/Menu.tsx",
            &with_defaults("@theme {}"),
        ),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["symbols"][0]["kind"], "ref");
    assert!(
        value["symbols"][0]["name"]
            .as_str()
            .unwrap()
            .starts_with("sw-a-")
    );
}
