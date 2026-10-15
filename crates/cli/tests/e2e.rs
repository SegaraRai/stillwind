//! End-to-end tests that run the compiled binary against real files.
//!
//! These tests cover what only the CLI can be wrong about: reading files, joining the
//! resource URLs relative to the entry, resolving package CSS through export
//! conditions, and the exit status and messages of argument failures. The compiler's
//! own behaviour is covered by the library crates, so the assertions here stay at the
//! level of the emitted JSON.

use std::path::PathBuf;
use std::process::Command;

/// Path of a checked-in fixture under the CLI's `tests/fixtures` directory.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Resource URLs are rebased from the source file's own directory.
///
/// The placeholder must be gone from the output, because a leftover placeholder would
/// ship a broken URL.
#[test]
fn cli_rebases_js_resources_from_the_source_directory() {
    let output = Command::new(env!("CARGO_BIN_EXE_stillwind"))
        .args([
            "compile",
            "--source-text",
            "import {style} from 'stillwind/react'; const x=style('bg-[url(./icon.svg)]');",
            "--source-path",
            fixture("nested/Component.tsx").to_str().unwrap(),
            "--theme",
            fixture("stillwind.css").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_eq!(value["lintSettings"]["macro-argument-mutation"], "deny");
    assert_eq!(value["lintSettings"]["ambiguous-base-order"], "warn");
    let css = value["css"].as_str().unwrap();
    assert!(css.contains("nested/icon.svg"), "{css}");
    assert!(!css.contains("__stillwind_"), "{css}");
}

/// A React module compiles to layered CSS and macro-free code.
#[test]
fn cli_compiles_a_react_module_to_layered_css_and_macro_free_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_stillwind"))
        .args([
            "compile",
            "--source",
            fixture("Button.tsx").to_str().unwrap(),
            "--theme",
            fixture("stillwind.css").to_str().unwrap(),
        ])
        .output()
        .expect("the stillwind CLI starts");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let css = value["css"].as_str().unwrap();
    let code = value["code"].as_str().unwrap();

    assert!(css.contains("@layer properties, theme, base, components, utilities;"));
    assert!(css.contains(".sw_"));
    assert!(css.contains(":hover{"));
    assert!(code.contains("{className:"));
    assert!(!code.contains("style(\"bg-accent"));
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

/// Argument and option failures exit with status 2 and a specific message.
#[test]
fn cli_reports_typed_argument_and_option_failures() {
    for (args, expected) in [
        (vec!["compile", "--source"], "missing value for --source"),
        (
            vec![
                "compile",
                "--source-text",
                "",
                "--theme-text",
                "",
                "--prefix",
                "bad",
            ],
            "prefix must start with `--` and end with `-`",
        ),
        (vec!["unknown"], "unknown command `unknown`"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_stillwind"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
}

/// CSS parsing failures retain the text input's virtual document and editor range.
#[test]
fn cli_reports_css_failure_with_source_range() {
    let output = Command::new(env!("CARGO_BIN_EXE_stillwind"))
        .args([
            "compile",
            "--source-text",
            "export const x = 1;",
            "--theme-text",
            "@theme {",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("stillwind-cli:theme:1:1-1:9: SW13002")
    );
}

/// A CSS package export with a `style` condition resolves through it.
///
/// The fixture package also exports a `default` condition pointing at a JavaScript
/// file, so this test would fail if resolution ignored conditions.
#[test]
fn cli_resolves_css_package_exports_with_style_conditions() {
    let directory = std::env::temp_dir().join(format!(
        "stillwind-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package = directory.join("node_modules/@fixture/css");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"@fixture/css","exports":{".":{"style":"./entry.css","default":"./wrong.js"}}}"#,
    )
    .unwrap();
    std::fs::write(package.join("entry.css"), "@stillwind-utility paint { color: rebeccapurple; background-image:url('./paint.svg'); } @layer utilities { @stillwind-slot utilities; }").unwrap();
    std::fs::write(directory.join("entry.css"), "@import '@fixture/css';").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_stillwind"))
        .args([
            "compile",
            "--source-text",
            "import { style } from 'stillwind/react'; const props = style('paint');",
            "--theme",
            directory.join("entry.css").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["diagnostics"], serde_json::json!([]));
    assert!(
        result["css"]
            .as_str()
            .unwrap()
            .contains("color:rebeccapurple;")
    );
    assert!(
        result["css"]
            .as_str()
            .unwrap()
            .contains("node_modules/@fixture/css/paint.svg")
    );
    assert!(
        !result["css"]
            .as_str()
            .unwrap()
            .contains("__stillwind_resource_")
    );
    std::fs::remove_dir_all(directory).unwrap();
}
