use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn cli_compiles_a_react_module_to_layered_css_and_macro_free_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_shorth"))
        .args([
            "compile",
            "--source",
            fixture("Button.tsx").to_str().unwrap(),
            "--theme",
            fixture("shorth.css").to_str().unwrap(),
        ])
        .output()
        .expect("the shorth CLI starts");

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
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
