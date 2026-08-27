#[test]
fn serializes_a_layered_compile_result_for_node_consumers() {
    let output = shorth_napi::compile_react_json(
        r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90");"#,
        "Button.tsx",
        "@theme { --color-accent: oklch(.6 .2 250); }",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(value["css"].as_str().unwrap().contains("@layer utilities{"));
    assert!(value["css"].as_str().unwrap().contains(":hover{"));
    assert!(value["code"].as_str().unwrap().contains("className"));
    assert_eq!(value["symbols"][0]["kind"], "class");
    assert!(
        value["symbols"][0]["name"]
            .as_str()
            .unwrap()
            .starts_with("_s_")
    );
}

#[test]
fn exposes_generated_symbols_for_build_wide_collision_checks() {
    let output = shorth_napi::compile_react_json(
        r#"import { style } from "stillwind/react";
const menu = style.handle();
const props = style(menu.anchor);"#,
        "src/Menu.tsx",
        "@theme {}",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["symbols"][0]["kind"], "handle");
    assert!(
        value["symbols"][0]["name"]
            .as_str()
            .unwrap()
            .starts_with("--sw-h-")
    );
}

#[test]
fn serializes_editor_positions_for_diagnostics() {
    let output = shorth_napi::compile_react_json(
        "import { style } from \"stillwind/react\";\nconst props = style(\"bg-accnet\");",
        "Button.tsx",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    let diagnostic = &value["diagnostics"][0];
    assert_eq!(diagnostic["file"], "Button.tsx");
    assert_eq!(diagnostic["start_line"], 1);
    assert!(diagnostic["start_utf16"].as_u64().unwrap() > 0);
}

#[test]
fn generates_theme_backed_react_type_augmentations() {
    let output =
        shorth_napi::generate_react_types("@theme { --color-accent: red; }".to_owned()).unwrap();

    assert!(output.contains("declare module \"stillwind/react\""));
    assert!(output.contains("\"bg-accent\": true"));
}
