use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

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
    let css = value["css"].as_str().unwrap();
    assert!(css.contains("nested/icon.svg"), "{css}");
    assert!(!css.contains("__stillwind_"), "{css}");
}

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

    assert!(css.contains("@layer theme, base, components, utilities;"));
    assert!(css.contains("._s_"));
    assert!(css.contains(":hover{"));
    assert!(code.contains("{className:"));
    assert!(!code.contains("style(\"bg-accent"));
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

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
