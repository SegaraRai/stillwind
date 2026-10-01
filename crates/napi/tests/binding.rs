//! End-to-end tests for the pure Rust entry points behind the Node binding.
//!
//! The binding itself is a thin conversion layer, so these tests drive
//! `compile_source_*`, [`summarize_source_input`], [`render_stylesheet`], and
//! [`generate_vocabulary_types`] directly and serialize results through serde. JSON
//! fields use snake case; native JavaScript objects use camel case. These tests check
//! that projected runtime imports stay inside their verified script realms,
//! invalid projection metadata fails the compile instead of producing partial code, and
//! options are validated rather than defaulted silently.

#[allow(dead_code)]
#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::error::Error;

use stillwind_compiler::{CompilerOptions, IdentifierKind, RejectedIdentifier};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_napi::{
    BindingApiError, BindingRejectedIdentifier, CompileCssInput, CompileSourceInput, ProjectSource,
    compile_project_input, compile_source_input, generate_vocabulary_types, render_stylesheet,
    summarize_source_input,
};
use stillwind_source::{ModuleId, SourceUri};

fn compile_input(source: &str, module_id: &str, theme: &str) -> CompileSourceInput {
    CompileSourceInput {
        source: source.into(),
        module_id: module_id.into(),
        uri: SourceUri::parse("stillwind-input:source").unwrap(),
        source_map_source: Some(module_id.into()),
        language: ScriptLanguage::Tsx,
        module_kind: ScriptModuleKind::Unambiguous,
        css: css_input(theme),
    }
}

fn css_input(content: &str) -> CompileCssInput {
    CompileCssInput {
        content: content.into(),
        name: "stillwind.css".into(),
        uri: SourceUri::parse("stillwind-input:theme").unwrap(),
        source_map_source: Some("stillwind.css".into()),
    }
}

#[test]
fn json_options_use_camel_case_and_enum_cases_follow_their_contracts() {
    let options: CompilerOptions = serde_json::from_value(serde_json::json!({
        "tailwindPrefix": "--compat-",
        "classNamespace": "application",
        "useCssIf": true,
        "hashLength": 8
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
        "common-js"
    );
    assert_eq!(
        serde_json::from_value::<ScriptModuleKind>(serde_json::json!("common-js")).unwrap(),
        ScriptModuleKind::CommonJs
    );
    assert_eq!(
        serde_json::to_value(stillwind_ir::AdapterKind::Vue).unwrap(),
        "vue"
    );
    assert_eq!(
        serde_json::to_value(stillwind_diagnostics::VariantInputErrorKind::CustomProperty).unwrap(),
        "customProperty"
    );
    assert_eq!(
        serde_json::to_value(stillwind_diagnostics::CompileDiagnostic::FrameworkConflict).unwrap(),
        "frameworkConflict"
    );
}

/// Malformed sources are reported through the same schema a host reads.
#[test]
fn rejected_names_are_transported_beside_symbols() {
    let rejected = RejectedIdentifier {
        kind: IdentifierKind::RefMarker,
        hash: "0123456789abcdef".to_owned(),
        reason: "no name for this declaration".to_owned(),
        substituted: true,
    };
    let native: BindingRejectedIdentifier = rejected.into();
    let value: serde_json::Value = serde_json::to_value(&native).unwrap();
    assert_eq!(value["kind"], "ref-marker");
    assert_eq!(value["hash"], "0123456789abcdef");
    assert_eq!(value["substituted"], true);
}

#[test]
fn lint_severities_keep_their_host_protocol_spellings() {
    for severity in ["warn", "deny", "off"] {
        let output = compile_source_input(&compile_input(
            "import {style} from 'stillwind/react'; const props = style('flex');",
            "entry.tsx",
            &format!("@utility flex {{display:flex;}} @stillwind-slot utilities; @stillwind {{valid-anchor-scope: {severity};}}"),
        ), &CompilerOptions::default()).unwrap();
        assert_eq!(output.lint_settings["valid-anchor-scope"], severity);
        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["lint_settings"]["valid-anchor-scope"], severity);
    }
}

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
        "module_id": "Component.svelte",
        "source": format!("{first}\n{second}"),
        "language": "ts",
        "source_format": "svelte",
        "framework": "svelte",
        "fw_code": original,
        "fw2js_map": {
            "kind": "volar",
            "mappings": [
                { "sourceOffsets": [a], "generatedOffsets": [0], "lengths": [first.len()] },
                { "sourceOffsets": [b], "generatedOffsets": [first.len() + 1], "lengths": [second.len()] },
            ],
        },
        "converter": {
            "name": "svelte2tsx",
            "version": "0.7.61",
            "options_fingerprint": "realm-test"
        },
        "import_realms": [
            { "id": 1, "start": a, "end": a + first.len() },
            { "id": 2, "start": b, "end": b + second.len() },
        ],
        "regions": [
            { "import_realm": 1, "start": a,"end": a + first.len() },
            { "import_realm": 2, "start": b,"end": b + second.len()},
        ],
    });
    let compile = |input: &serde_json::Value| -> serde_json::Value {
        serde_json::to_value(
            compile_project_input(
                &sources_from_value(serde_json::json!([input])),
                &css_input("@utility flex {display:flex;} @stillwind-slot utilities;"),
                &CompilerOptions::default(),
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
    assert_eq!(invalid["modules"][0]["diagnostics"][0]["code"], "SW11008");
    input["regions"] = serde_json::json!([]);
    let missing = compile(&input);
    assert!(missing["modules"][0]["code"].is_null());
    assert_eq!(missing["modules"][0]["diagnostics"][0]["code"], "SW11007");
}

#[test]
fn projected_project_input_edits_the_sfc_and_reports_original_coordinates() {
    let script = "import {style} from 'stillwind/svelte'; const props = style('flex');";
    let prefix = "<!-- 🧭 -->\n<script>";
    let suffix = "</script>\n<div {...props}></div><style>/* preserved */</style>";
    let original = format!("{prefix}{script}{suffix}");
    let source = serde_json::json!({
        "module_id": "Component.svelte",
        "source": script,
        "language": "ts",
        "source_format": "svelte",
        "framework": "svelte",
        "fw_code": original,
        "fw2js_map": {
            "kind": "volar",
            "mappings": [{
                "sourceOffsets": [prefix.encode_utf16().count()],
                "generatedOffsets": [0],
                "lengths": [script.len()],
            }],
        },
        "converter": {
            "name": "svelte2tsx",
            "version": "0.7.61",
            "options_fingerprint": "test",
        },
        "import_realms": [{
            "id": 0,
            "start": prefix.encode_utf16().count(),
            "end": prefix.encode_utf16().count() + script.len()
        }],
        "regions": [{
            "import_realm": 0,
            "start": prefix.encode_utf16().count(),
            "end": prefix.encode_utf16().count() + script.len()
        }],
    });
    let output = compile_project_input(
        &sources_from_value(serde_json::json!([source])),
        &css_input("@utility flex {display:flex;} @stillwind-slot utilities;"),
        &CompilerOptions::default(),
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
    let output = compile_source_input(
        &compile_input(
            source,
            "Button.tsx",
            &css_fixture::with_defaults("@theme { --color-accent: oklch(.6 .2 250); }"),
        ),
        &CompilerOptions::default(),
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
            .starts_with("sw_")
    );
}

#[test]
fn exposes_generated_symbols_for_build_wide_collision_checks() {
    let output = compile_source_input(
        &compile_input(
            r#"import { style } from "stillwind/react";
const menu = style.ref();
const props = style(menu.asAnchor());"#,
            "src/Menu.tsx",
            &css_fixture::with_defaults("@theme {}"),
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

#[test]
fn serializes_cross_module_ref_summaries() {
    let output = summarize_source_input(
        r#"import { style } from "stillwind/react";
export const menu = style.ref("menu");
export const trigger = style.ref(menu, "trigger");"#,
        &ModuleId::from(r"src\Parent.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["schema"], 3);
    assert_eq!(value["module_id"], r"src\Parent.tsx");
    assert_eq!(value["exports"][0]["name"], "menu");
    assert_eq!(value["exports"][0]["value"]["kind"], "refs");
    assert_eq!(
        value["exports"][0]["value"]["value"][0]["permitted_boundaries"],
        serde_json::Value::Null
    );
    assert_eq!(value["exports"][1]["name"], "trigger");
    assert_eq!(value["exports"][1]["value"]["kind"], "refs");
    // The bounded ref keeps the boundary it was declared with, so a consumer can
    // reconstruct the pair from the summary alone.
    assert_eq!(
        value["exports"][1]["value"]["value"][0]["permitted_boundaries"]["kind"],
        "ref"
    );
}

#[test]
fn requires_typescript_facts_for_cross_module_refs() {
    let sources = serde_json::json!([
        {
            "module_id": "src/Parent.tsx",
            "source": r#"import { style } from "stillwind/react";
export const menu = style.ref("menu");
export const parent = style(menu);"#
        },
        {
            "module_id": "src/Child.tsx",
            "source": r#"import { style } from "stillwind/react";
import { menu } from "./Parent";
export const child = style({ [menu`hover:`]: "text-white" });"#
        }
    ]);
    let output = compile_project_input(
        &sources_from_value(sources),
        &css_input(&css_fixture::with_defaults(
            "@theme { --color-white: white; }",
        )),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["modules"].as_array().unwrap().len(), 2);
    assert_eq!(value["modules"][1]["diagnostics"][0]["code"], "SW03007");
}

#[test]
fn assembles_project_fragments_under_one_shared_stylesheet() {
    let sources = serde_json::json!([
        {
            "module_id": "src/One.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const one = style("text-white");"#
        },
        {
            "module_id": "src/Two.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const two = style("bg-white");"#
        },
        {
            "module_id": "src/Duplicate.tsx",
            "source": r#"import { style } from "stillwind/react";
    export const duplicate = style("text-white");"#
        }
    ]);
    let theme = css_fixture::with_defaults(
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
fn serializes_editor_positions_for_diagnostics() {
    let output = compile_source_input(
        &compile_input(
            "import { style } from \"stillwind/react\";\nconst props = style(\"bg-accnet\");",
            "Button.tsx",
            &css_fixture::with_defaults("@theme { --color-accent: red; }"),
        ),
        &CompilerOptions::default(),
    )
    .unwrap();

    let value: serde_json::Value = serde_json::to_value(&output).unwrap();
    let diagnostic = &value["lint_findings"][0];
    assert!(diagnostic.get("file").is_none());
    assert!(diagnostic["start"].as_u64().unwrap() > 0);
    assert_eq!(diagnostic["uri"], "stillwind-input:source");
    assert_eq!(diagnostic["start_line"], 1);
    assert!(
        diagnostic["end_column"].as_u64().unwrap() > diagnostic["start_column"].as_u64().unwrap()
    );
    let report = &value["reports"][0];
    assert_eq!(report["primary"]["location"]["uri"], diagnostic["uri"]);
    assert_eq!(
        report["primary"]["location"]["revision"],
        diagnostic["revision"]
    );
    assert!(report["actions"].is_array());
}

#[test]
fn generates_theme_backed_shared_vocabulary_augmentations() {
    let output = generate_vocabulary_types(css_fixture::with_defaults(
        "@theme { --color-accent: red; }",
    ))
    .unwrap();

    assert!(output.contains("declare module \"stillwind/macro\""));
    assert!(output.contains("\"bg-accent\": true"));
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

fn sources_from_value(mut value: serde_json::Value) -> Vec<ProjectSource> {
    for source in value.as_array_mut().unwrap() {
        let path = source["module_id"].as_str().unwrap().to_owned();
        let uri = format!("test:///{path}");
        source["uri"] = serde_json::Value::String(uri.clone());
        source["source_map_source"] = serde_json::Value::String(uri);
        if source["language"].is_null() {
            source["language"] = serde_json::Value::String("tsx".into());
        }
        source["module_kind"] = serde_json::Value::String("unambiguous".into());
    }
    serde_json::from_value(value).unwrap()
}
