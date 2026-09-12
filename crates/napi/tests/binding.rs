//! End-to-end tests for the pure Rust entry points behind the Node binding.
//!
//! The binding itself is a thin conversion layer, so these tests drive
//! `compile_react_*`, [`summarize_react_input`], [`render_react_stylesheet`], and
//! [`generate_react_types`] directly and serialize the result the way a JavaScript
//! caller would see it. They pin the transport contract that is easy to break without a
//! JavaScript test: projected runtime imports stay inside their verified script realms,
//! invalid projection metadata fails the compile instead of producing partial code, and
//! options are validated rather than defaulted silently.

use std::error::Error;

use stillwind_compiler::CompilerOptions;
use stillwind_napi::{
    BindingApiError, ProjectSource, compile_react_input, compile_react_project_input,
    compile_react_project_input_with_options, generate_react_types, render_react_stylesheet,
    summarize_react_input,
};

#[test]
fn projected_runtime_imports_stay_in_their_verified_script_realms() {
    let first = "import {style as sm} from 'stillwind/svelte'; const a = sm('flex', {class: x});";
    let second = "import {style as si} from 'stillwind/svelte'; const b = si('flex', {class: y});";
    let prefix = "<!-- 🧭 -->\n<script context=\"module\">";
    let middle = "</script>\n<script>";
    let suffix = "</script>\n<div {...b}></div>";
    let original = format!("{prefix}{first}{middle}{second}{suffix}");
    let a = prefix.encode_utf16().count();
    let b = a + first.len() + middle.len();
    let mut input = serde_json::json!({
        "path":"Component.svelte", "source":format!("{first}\n{second}"),
        "language":"ts", "framework":"Svelte", "fw_code": original,
        "fw2js_map":{"kind":"volar","mappings":[
            {"sourceOffsets":[a],"generatedOffsets":[0],"lengths":[first.len()]},
            {"sourceOffsets":[b],"generatedOffsets":[first.len()+1],"lengths":[second.len()]}
        ]},
        "converter":{"name":"svelte2tsx","version":"0.7.61","options_fingerprint":"realm-test"},
        "import_realms":[{"id":1,"start":a,"end":a+first.len()},{"id":2,"start":b,"end":b+second.len()}],
        "regions":[{"import_realm":1,"start":a,"end":a+first.len()},{"import_realm":2,"start":b,"end":b+second.len()}]
    });
    let compile = |input: &serde_json::Value| -> serde_json::Value {
        serde_json::to_value(
            compile_react_project_input(
                &sources_from_value(serde_json::json!([input])),
                "@utility flex {display:flex;} @stillwind-slot utilities;",
            )
            .unwrap(),
        )
        .unwrap()
    };
    let output = compile(&input);
    let module = &output["modules"][0];
    assert_eq!(module["diagnostics"], serde_json::json!([]));
    let code = module["code"].as_str().unwrap();
    let runtime = "import { stillwindClsx } from \"stillwind/svelte/runtime\";";
    assert!(code.starts_with(&format!("{prefix}{runtime}")), "{code}");
    assert!(
        code.contains(&format!(
            "{middle}import {{ stillwindClsx as stillwindClsx_1 }}"
        )),
        "{code}"
    );
    assert!(code.ends_with(suffix));
    assert_eq!(code.matches("stillwind/svelte/runtime").count(), 2);
    assert!(!code.contains("style as"));
    let duplicate = input["regions"][0].clone();
    input["regions"].as_array_mut().unwrap().push(duplicate);
    let invalid = compile(&input);
    assert!(invalid["modules"][0]["code"].is_null());
    assert_eq!(
        invalid["modules"][0]["diagnostics"][0]["code"],
        "SHIMPORT001"
    );
    input["regions"] = serde_json::json!([]);
    let missing = compile(&input);
    assert!(missing["modules"][0]["code"].is_null());
    assert_eq!(
        missing["modules"][0]["diagnostics"][0]["code"],
        "SHIMPORT001"
    );
}

#[test]
fn projected_project_input_edits_the_sfc_and_reports_original_coordinates() {
    let script = "import {style} from 'stillwind/svelte'; const props = style('flex');";
    let prefix = "<!-- 🧭 -->\n<script>";
    let suffix = "</script>\n<div {...props}></div><style>/* preserved */</style>";
    let original = format!("{prefix}{script}{suffix}");
    let source = serde_json::json!({
        "path":"Component.svelte", "source":script, "language":"ts", "framework":"Svelte",
        "fw_code":original,
        "fw2js_map":{"kind":"volar","mappings":[{"sourceOffsets":[prefix.encode_utf16().count()],"generatedOffsets":[0],"lengths":[script.len()]}]},
        "converter":{"name":"svelte2tsx","version":"0.7.61","options_fingerprint":"test"},
        "import_realms":[{"id":0,"start":prefix.encode_utf16().count(),"end":prefix.encode_utf16().count()+script.len()}],
        "regions":[{"import_realm":0,"start":prefix.encode_utf16().count(),"end":prefix.encode_utf16().count()+script.len()}]
    });
    let output = compile_react_project_input(
        &sources_from_value(serde_json::json!([source])),
        "@utility flex {display:flex;} @stillwind-slot utilities;",
    )
    .unwrap();
    let result: serde_json::Value = serde_json::to_value(&output).unwrap();
    let module = &result["modules"][0];
    assert_eq!(module["diagnostics"], serde_json::json!([]));
    let code = module["code"].as_str().unwrap();
    assert!(code.starts_with(prefix));
    assert!(code.ends_with(suffix));
    assert!(code.contains("{class:"));
    assert!(!code.contains("style('flex')"));
    let expected = prefix.encode_utf16().count() + script.find("style('").unwrap();
    assert_eq!(
        module["mappings"][0]["original_start"],
        prefix.encode_utf16().count()
    );
    assert_eq!(module["mappings"][1]["original_start"], expected);
    assert!(!code.contains("stillwind/svelte"));
}

#[test]
fn serializes_a_layered_compile_result_for_node_consumers() {
    let source = r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90");"#;
    let output = compile_react_input(
        source,
        "Button.tsx",
        &css_fixture::with_defaults("@theme { --color-accent: oklch(.6 .2 250); }"),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert!(value["css"].as_str().unwrap().contains("@layer utilities{"));
    assert!(value["css"].as_str().unwrap().contains(":hover{"));
    assert!(value["code"].as_str().unwrap().contains("className"));
    assert_eq!(value["mappings"].as_array().unwrap().len(), 2);
    assert_eq!(value["mappings"][0]["original_start"], 0);
    assert_eq!(
        value["mappings"][1]["original_start"],
        u64::try_from(source.find("style(\"").unwrap()).unwrap()
    );
    assert!(
        value["mappings"][1]["generated_end"].as_u64().unwrap()
            > value["mappings"][1]["generated_start"].as_u64().unwrap()
    );
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
    let output = compile_react_input(
        r#"import { style } from "stillwind/react";
const menu = style.ref();
const props = style(menu.anchor);"#,
        "src/Menu.tsx",
        &css_fixture::with_defaults("@theme {}"),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
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
    let output = summarize_react_input(
        r#"import { style } from "stillwind/react";
export const menu = style.scope("menu");
export const trigger = style.ref("trigger");"#,
        r"src\Parent.tsx",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["schema"], 1);
    assert_eq!(value["module_path"], "src/Parent.tsx");
    assert_eq!(value["exports"][0]["name"], "menu");
    assert_eq!(value["exports"][0]["value"]["kind"], "scopes");
    assert_eq!(value["exports"][1]["name"], "trigger");
    assert_eq!(value["exports"][1]["value"]["kind"], "refs");
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
export const child = style({ [menu`hover:`]: "text-white" });"#
        }
    ]);
    let output = compile_react_project_input(
        &sources_from_value(sources),
        &css_fixture::with_defaults("@theme { --color-white: white; }"),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
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
    let theme = css_fixture::with_defaults(
        r#"@import "stillwind";
@theme { --color-white: white; }"#,
    );
    let output = compile_react_project_input(&sources_from_value(sources), &theme).unwrap();
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
    let stylesheet = render_react_stylesheet(fragments.join("\n"), theme.to_owned(), None).unwrap();
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
    let output = compile_react_input(
        "import { style } from \"stillwind/react\";\nconst props = style(\"bg-accnet\");",
        "Button.tsx",
        &css_fixture::with_defaults("@theme { --color-accent: red; }"),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    let diagnostic = &value["lint_findings"][0];
    assert_eq!(diagnostic["file"], "Button.tsx");
    assert!(diagnostic["start"].as_u64().unwrap() > 0);
}

#[test]
fn generates_theme_backed_react_type_augmentations() {
    let output = generate_react_types(css_fixture::with_defaults(
        "@theme { --color-accent: red; }",
    ))
    .unwrap();

    assert!(output.contains("declare module \"stillwind/react\""));
    assert!(output.contains("\"bg-accent\": true"));
}

#[test]
fn project_api_preserves_configuration_errors() {
    let options = CompilerOptions {
        prefix: "invalid".into(),
        ..CompilerOptions::default()
    };
    let error = compile_react_project_input_with_options(&[], "", &options)
        .err()
        .unwrap();
    assert!(matches!(error, BindingApiError::Options(_)));
    assert!(error.source().is_some());
}

fn sources_from_value(value: serde_json::Value) -> Vec<ProjectSource> {
    serde_json::from_value(value).unwrap()
}

#[allow(dead_code)]
#[path = "../../test-support/css.rs"]
mod css_fixture;
