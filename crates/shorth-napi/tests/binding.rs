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
const menu = style.ref();
const props = style(menu.anchor);"#,
        "src/Menu.tsx",
        "@theme {}",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["symbols"][0]["kind"], "ref");
    assert!(
        value["symbols"][0]["name"]
            .as_str()
            .unwrap()
            .starts_with("--sw-r-")
    );
}

#[test]
fn serializes_cross_module_handle_summaries() {
    let output = shorth_napi::summarize_react_json(
        r#"import { style } from "stillwind/react";
export const menu = style.scope("menu");
export const trigger = style.ref("trigger");"#,
        r"src\Parent.tsx",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["schema"], 1);
    assert_eq!(value["module_path"], "src/Parent.tsx");
    assert_eq!(value["exports"]["menu"]["kind"], "scopes");
    assert_eq!(value["exports"]["trigger"]["kind"], "refs");
}

#[test]
fn requires_typescript_facts_for_cross_module_refs() {
    let sources = serde_json::json!([
        {
            "path": "src/Parent.tsx",
            "source": r#"import { style } from "stillwind/react";
export const menu = style.ref("menu");
export const parent = style(menu);"#
        },
        {
            "path": "src/Child.tsx",
            "source": r#"import { style } from "stillwind/react";
import { menu } from "./Parent";
export const child = style({ [menu`&:hover`]: "text-white" });"#
        }
    ]);
    let output = shorth_napi::compile_react_project_json(
        &sources.to_string(),
        "@theme { --color-white: white; }",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["modules"].as_array().unwrap().len(), 2);
    assert_eq!(value["modules"][1]["diagnostics"][0]["code"], "SHTS001");
}

#[test]
fn assembles_project_fragments_under_one_shared_stylesheet() {
    let sources = serde_json::json!([
        {
            "path": "src/One.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const one = style("text-white");"#
        },
        {
            "path": "src/Two.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const two = style("bg-white");"#
        },
        {
            "path": "src/Duplicate.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const duplicate = style("text-white");"#
        }
    ]);
    let theme = r#"@import "tailwindcss";
@theme { --color-white: white; }"#;
    let output = shorth_napi::compile_react_project_json(&sources.to_string(), theme).unwrap();
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    let fragments = value["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|module| module["css"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert!(
        fragments
            .iter()
            .all(|fragment| !fragment.contains("@layer"))
    );
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.matches("{color:var(--color-white)").count())
            .sum::<usize>(),
        1
    );
    let stylesheet =
        shorth_napi::render_react_stylesheet(fragments.join("\n"), theme.to_owned(), None).unwrap();
    assert_eq!(
        stylesheet
            .matches("@layer theme, base, components, utilities;")
            .count(),
        1
    );
    assert_eq!(stylesheet.matches("@layer base{").count(), 1);
    assert!(stylesheet.contains("color:var(--color-white)"));
    assert!(stylesheet.contains("background-color:var(--color-white)"));
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
