//! Checks lint severity and diagnostic editor coordinates.

use super::*;

#[test]
fn lint_severities_keep_their_host_protocol_spellings() {
    for severity in ["warn", "deny", "off"] {
        let output = compile_source_input(
            &compile_input(
                "import {style} from 'stillwind/react'; const props = style('flex');",
                "entry.tsx",
                "@utility flex {display:flex;} @stillwind-slot utilities;",
            ),
            &CompilerOptions {
                lint: LintOverrides {
                    valid_anchor_scope: Some(LintSeverity::parse(severity).unwrap()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output.lint_settings["valid-anchor-scope"], severity);
        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["lintSettings"]["valid-anchor-scope"], severity);
    }
}

#[test]
fn serializes_editor_positions_for_diagnostics() {
    let output = compile_source_input(
        &compile_input(
            "import { style } from \"stillwind/react\";\nconst props = style(\"bg-accnet\");",
            "Button.tsx",
            &with_defaults("@theme { --color-accent: red; }"),
        ),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    let diagnostic = &value["lintFindings"][0];
    assert!(diagnostic.get("file").is_none());
    assert!(diagnostic["start"].as_u64().unwrap() > 0);
    assert_eq!(diagnostic["uri"], "stillwind-input:source");
    assert_eq!(diagnostic["startLine"], 1);
    assert!(
        diagnostic["endColumn"].as_u64().unwrap() > diagnostic["startColumn"].as_u64().unwrap()
    );
    let report = &value["reports"][0];
    assert_eq!(report["primary"]["location"]["uri"], diagnostic["uri"]);
    assert_eq!(
        report["primary"]["location"]["revision"],
        diagnostic["revision"]
    );
    assert!(report["actions"].is_array());
}
