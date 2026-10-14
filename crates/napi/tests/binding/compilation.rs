//! Checks serialized compilation, summaries, graph output, and options.

use super::*;

#[test]
fn json_options_use_camel_case_and_enum_cases_follow_their_contracts() {
    let options: CompilerOptions = serde_json::from_value(serde_json::json!({
        "tailwindPrefix": "--compat-",
        "classNamespace": "application",
        "useCssIf": true,
        "hashLength": 8,
    }))
    .unwrap();
    assert_eq!(options.tailwind_prefix, "--compat-");
    assert_eq!(options.class_namespace, "application");
    assert!(options.use_css_if);
    assert_eq!(options.hash_length, 8);
    let json = serde_json::to_value(&options).unwrap();
    assert_eq!(json["tailwindPrefix"], "--compat-");
    assert!(json.get("tailwind_prefix").is_none());
    let design = serde_json::to_value(options.design_system()).unwrap();
    assert_eq!(design["tailwindPrefix"], "--compat-");
    assert!(design.get("tailwind_prefix").is_none());
    assert_eq!(
        serde_json::to_value(ScriptModuleKind::CommonJs).unwrap(),
        "commonjs"
    );
    assert_eq!(
        serde_json::from_value::<ScriptModuleKind>(serde_json::json!("commonjs")).unwrap(),
        ScriptModuleKind::CommonJs
    );
    assert_eq!(serde_json::to_value(AdapterKind::Vue).unwrap(), "vue");
    assert_eq!(
        serde_json::to_value(VariantInputErrorKind::CustomProperty).unwrap(),
        "customProperty"
    );
    assert_eq!(
        serde_json::to_value(CompileDiagnostic::FrameworkConflict).unwrap(),
        "frameworkConflict"
    );
}

#[test]
fn serializes_a_layered_compile_result_for_node_consumers() {
    let source = r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90");"#;
    let output = compile_source_input(
        &compile_input(
            source,
            "Button.tsx",
            &with_defaults("@theme { --color-accent: oklch(.6 .2 250); }"),
        ),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert!(value["css"].as_str().unwrap().contains("@layer utilities{"));
    assert!(value["css"].as_str().unwrap().contains(":hover{"));
    assert!(value["code"].as_str().unwrap().contains("className"));
    assert_eq!(value["mappings"].as_array().unwrap().len(), 2);
    assert_eq!(value["mappings"][0]["originalStart"], 0);
    assert_eq!(
        value["mappings"][1]["originalStart"],
        u64::try_from(source.find("style(\"").unwrap()).unwrap()
    );
    assert!(
        value["mappings"][1]["generatedEnd"].as_u64().unwrap()
            > value["mappings"][1]["generatedStart"].as_u64().unwrap()
    );
    assert_eq!(value["symbols"][0]["kind"], "class");
    assert!(
        value["symbols"][0]["name"]
            .as_str()
            .unwrap()
            .starts_with("sw_")
    );
}

#[test]
fn serializes_cross_module_ref_summaries() {
    let output = summarize_source_input(
        r#"import { style } from "stillwind/react";
export const menu = style.ref("menu");
export const trigger = style.ref("trigger");"#,
        &ModuleId::from(r"src\Parent.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["schema"], 4);
    assert_eq!(value["moduleId"], r"src\Parent.tsx");
    assert_eq!(value["exports"][0]["name"], "menu");
    assert_eq!(value["exports"][0]["value"]["kind"], "refs");
    assert_eq!(value["exports"][1]["name"], "trigger");
    for (index, binding) in [(0, "menu"), (1, "trigger")] {
        let exported = &value["exports"][index]["value"];
        assert_eq!(exported["kind"], "refs");
        assert_eq!(exported["value"]["kind"], "ref");
        let candidates = &exported["value"]["candidates"];
        assert_eq!(candidates["kind"], "finite");
        assert_eq!(candidates["values"].as_array().unwrap().len(), 1);
        assert_eq!(candidates["values"][0]["bindingName"], binding);
        assert_eq!(candidates["values"][0]["moduleId"], r"src\Parent.tsx");
    }
}

#[test]
fn requires_typescript_facts_for_cross_module_refs() {
    let sources = serde_json::json!([
        {
            "moduleId": "src/Parent.tsx",
            "kind": "direct",
            "syntax": {
                "language": "tsx",
                "moduleKind": "unambiguous",
            },
            "document": {
                "format": "jsx",
                "code": r#"import { style } from "stillwind/react";
export const menu = style.ref("menu");
export const parent = style(menu);"#,
            }
        },
        {
            "moduleId": "src/Child.tsx",
            "kind": "direct",
            "syntax": {
                "language": "tsx",
                "moduleKind": "unambiguous",
            },
            "document": {
                "format": "jsx",
                "code": r#"import { style } from "stillwind/react";
import { menu } from "./Parent";
export const child = style({ [menu`hover:`]: "text-white" });"#,
            },
        },
    ]);
    let output = compile_project_input(
        &sources_from_value(sources),
        &css_input(&with_defaults("@theme { --color-white: white; }")),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["modules"].as_array().unwrap().len(), 2);
    assert_eq!(value["modules"][1]["diagnostics"][0]["code"], "SW03001");
}

#[test]
fn assembles_project_fragments_under_one_shared_stylesheet() {
    let sources = serde_json::json!([
        {
            "moduleId": "src/One.tsx",
            "kind": "direct",
            "syntax": {
                "language": "tsx",
                "moduleKind": "unambiguous",
            },
            "document": {
                "format": "jsx",
                "code": r#"import { style } from "stillwind/react";
    export const one = style("text-white");"#,
            },
        },
        {
            "moduleId": "src/Two.tsx",
            "kind": "direct",
            "syntax": {
                "language": "tsx",
                "moduleKind": "unambiguous",
            },
            "document": {
                "format": "jsx",
                "code": r#"import { style } from "stillwind/react";
    export const two = style("bg-white");"#,
            },
        },
        {
            "moduleId": "src/Duplicate.tsx",
            "kind": "direct",
            "syntax": {
                "language": "tsx",
                "moduleKind": "unambiguous",
            },
            "document": {
                "format": "jsx",
                "code": r#"import { style } from "stillwind/react";
    export const duplicate = style("text-white");"#,
            },
        },
    ]);
    let theme = with_defaults(
        r#"@import "stillwind";
@theme { --color-white: white; }"#,
    );
    let output = compile_project_input(
        &sources_from_value(sources),
        &css_input(&theme),
        &CompilerOptions::default(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
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
        2
    );
    let stylesheet = render_stylesheet(fragments.join("\n"), theme.clone(), None).unwrap();
    assert_eq!(
        stylesheet
            .matches("@layer properties, theme, base, components, utilities;")
            .count(),
        1
    );
    assert_eq!(stylesheet.matches("@layer base{").count(), 1);
    assert!(stylesheet.contains("color:var(--color-white)"));
    assert!(stylesheet.contains("background-color:var(--color-white)"));
}

#[test]
fn project_api_preserves_configuration_errors() {
    let options = CompilerOptions {
        prefix: "invalid".into(),
        ..CompilerOptions::default()
    };
    let error = compile_project_input(&[], &css_input(""), &options)
        .err()
        .unwrap();
    assert!(matches!(error, BindingApiError::Options(_)));
    assert!(error.source().is_some());
}
