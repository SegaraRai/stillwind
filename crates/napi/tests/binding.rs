//! End-to-end tests for the pure Rust entry points behind the Node binding.
//!
//! The binding itself is a thin conversion layer, so these tests drive
//! `compile_source_*`, [`summarize_source_input`], [`render_stylesheet`], and
//! [`generate_vocabulary_types`] directly and serialize results through serde. JSON
//! fields and native JavaScript properties use camelCase. These tests check
//! that projected runtime imports stay inside their verified script realms,
//! invalid projection metadata fails the compile instead of producing partial code, and
//! options are validated rather than defaulted silently.

#[allow(dead_code)]
#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::error::Error;

use stillwind_compiler::{CompilerOptions, IdentifierKind, RejectedIdentifier};
use stillwind_diagnostics::{CompileDiagnostic, VariantInputErrorKind};
use stillwind_ir::{AdapterKind, ScriptLanguage, ScriptModuleKind};
use stillwind_napi::{
    BindingApiError, BindingRejectedIdentifier, CompileCssInput, CompileSourceInput,
    HostPreparedSource, compile_project_input, compile_source_input, generate_vocabulary_types,
    render_stylesheet, summarize_source_input,
};
use stillwind_source::{ModuleId, SourceRegistry, SourceUri};

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
        assert_eq!(json["lintSettings"]["valid-anchor-scope"], severity);
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
        "moduleId": "Component.svelte",
        "kind": "projected",
        "document": {
            "code": original,
            "format": "svelte",
        },
        "framework": "svelte",
        "projection": {
            "code": format!("{first}\n{second}"),
            "syntax": {
                "language": "ts",
                "moduleKind": "unambiguous",
            },
            "map": {
                "kind": "volar",
                "mappings": [
                    { "sourceOffsets": [a], "generatedOffsets": [0], "lengths": [first.len()] },
                    { "sourceOffsets": [b], "generatedOffsets": [first.len() + 1], "lengths": [second.len()] },
                ],
            },
            "converter": {
                "name": "svelte2tsx",
                "version": "0.7.61",
                "optionsFingerprint": "realm-test"
            },
            "importRealms": [
                { "id": 1, "start": a, "end": a + first.len() },
                { "id": 2, "start": b, "end": b + second.len() },
            ],
            "editableRegions": [
                { "importRealm": 1, "start": a,"end": a + first.len() },
                { "importRealm": 2, "start": b,"end": b + second.len() },
            ],
        },
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
    let duplicate = input["projection"]["editableRegions"][0].clone();
    input["projection"]["editableRegions"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let invalid = compile_project_input(
        &sources_from_value(serde_json::json!([input])),
        &css_input("@utility flex {display:flex;} @stillwind-slot utilities;"),
        &CompilerOptions::default(),
    )
    .err()
    .expect("overlapping editable regions must reject the input");
    assert_eq!(
        invalid.to_string(),
        "invalid source input: editable regions 0 and 2 overlap"
    );
    input["projection"]["editableRegions"] = serde_json::json!([]);
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
        "moduleId": "Component.svelte",
        "kind": "projected",
        "document": {
            "code": original,
            "format": "svelte",
        },
        "framework": "svelte",
        "projection": {
            "code": script,
            "syntax": {
                "language": "ts",
                "moduleKind": "unambiguous",
            },
            "map": {
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
                "optionsFingerprint": "test",
            },
            "importRealms": [{
                "id": 0,
                "start": prefix.encode_utf16().count(),
                "end": prefix.encode_utf16().count() + script.len(),
            }],
            "editableRegions": [{
                "importRealm": 0,
                "start": prefix.encode_utf16().count(),
                "end": prefix.encode_utf16().count() + script.len(),
            }],
        },
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
        module["mappings"][0]["originalStart"],
        prefix.encode_utf16().count()
    );
    assert_eq!(module["mappings"][1]["originalStart"], expected);
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

fn sources_from_value(mut value: serde_json::Value) -> Vec<HostPreparedSource> {
    for source in value.as_array_mut().unwrap() {
        let path = source["moduleId"].as_str().unwrap().to_owned();
        let uri = format!("test:///{path}");
        source["document"]["uri"] = serde_json::Value::String(uri.clone());
        source["document"]["sourceMapSource"] = serde_json::Value::String(uri);
    }
    serde_json::from_value(value).unwrap()
}

fn direct_prepared_value() -> serde_json::Value {
    serde_json::json!({
        "kind": "direct",
        "moduleId": "entry.ts",
        "document": {
            "uri": "test:///entry.ts",
            "code": "const value = 1;",
            "format": "script",
            "sourceMapSource": "entry.ts",
        },
        "syntax": {
            "language": "ts",
            "moduleKind": "commonjs",
        },
    })
}

fn projected_prepared_value() -> serde_json::Value {
    serde_json::json!({
        "kind": "projected",
        "moduleId": "entry.svelte",
        "document": {
            "uri": "test:///entry.svelte",
            "code": "😀<script>const value = 1;</script>",
            "format": "svelte",
            "sourceMapSource": "entry.svelte",
        },
        "projection": {
            "code": "const value = 1;",
            "syntax": {
                "language": "ts",
                "moduleKind": "module",
            },
            "map": {
                "kind": "volar",
                "mappings": [],
            },
            "converter": {
                "name": "test",
                "version": "1",
                "optionsFingerprint": "test",
            },
            "importRealms": [
                { "id": 0, "start": 10, "end": 26 },
            ],
            "editableRegions": [
                { "importRealm": 0, "start": 10, "end": 26 },
            ],
        }
    })
}

#[test]
fn prepared_source_requires_nested_document_syntax_and_projection_metadata() {
    for (value, paths) in [
        (
            direct_prepared_value(),
            vec![
                vec!["kind"],
                vec!["moduleId"],
                vec!["document"],
                vec!["syntax"],
                vec!["document", "uri"],
                vec!["document", "code"],
                vec!["document", "format"],
                vec!["syntax", "language"],
                vec!["syntax", "moduleKind"],
            ],
        ),
        (
            projected_prepared_value(),
            vec![
                vec!["projection"],
                vec!["projection", "code"],
                vec!["projection", "syntax"],
                vec!["projection", "map"],
                vec!["projection", "converter"],
                vec!["projection", "importRealms"],
                vec!["projection", "editableRegions"],
            ],
        ),
    ] {
        assert!(serde_json::from_value::<HostPreparedSource>(value.clone()).is_ok());
        for path in paths {
            let mut missing = value.clone();
            let (field, parents) = path.split_last().unwrap();
            let parent = parents
                .iter()
                .fold(&mut missing, |object, field| &mut object[*field]);
            parent.as_object_mut().unwrap().remove(*field);
            assert!(
                serde_json::from_value::<HostPreparedSource>(missing).is_err(),
                "{path:?}"
            );
        }
    }
    assert!(
        serde_json::from_value::<HostPreparedSource>(serde_json::json!({
            "uri": "test:///entry.ts",
            "moduleId": "entry.ts",
            "source": "",
            "language": "ts",
            "moduleKind": "module",
        }))
        .is_err()
    );
}

#[test]
fn prepared_source_rejects_document_formats_without_matching_preparation() {
    for (format, core_format) in [("astro", "Astro"), ("svelte", "Svelte"), ("vue", "Vue")] {
        let mut value = direct_prepared_value();
        value["document"]["format"] = format.into();
        let source: HostPreparedSource = serde_json::from_value(value).unwrap();
        let error = source.input(&mut SourceRegistry::default()).err().unwrap();
        assert_eq!(
            error.reason,
            format!("direct source requires script or JSX format, received {core_format}")
        );
    }
    for (format, core_format) in [("script", "Script"), ("jsx", "Jsx")] {
        let mut value = projected_prepared_value();
        value["document"]["format"] = format.into();
        let source: HostPreparedSource = serde_json::from_value(value).unwrap();
        let error = source.input(&mut SourceRegistry::default()).err().unwrap();
        assert_eq!(
            error.reason,
            format!("projected source requires a component format, received {core_format}")
        );
    }
}

#[test]
fn prepared_documents_preserve_upstream_maps_source_names_and_parser_settings() {
    let upstream = serde_json::json!({
        "version": 3,
        "sources": ["upstream.ts"],
        "sourcesContent": ["const value = 1;"],
        "names": [],
        "mappings": "AAAA",
    })
    .to_string();
    for mut value in [direct_prepared_value(), projected_prepared_value()] {
        value["document"]["upstreamMap"] = upstream.clone().into();
        value["stylesheetImports"] = serde_json::json!(["virtual:styles"]);
        value["moduleCssImport"] = "virtual:module-css".into();
        value["framework"] = "svelte".into();
        let source: HostPreparedSource = serde_json::from_value(value.clone()).unwrap();
        assert!(source.type_facts().is_empty());
        let input = source.input(&mut SourceRegistry::default()).unwrap();
        assert_eq!(input.document().upstream_map(), Some(upstream.as_str()));
        assert_eq!(
            input.module_id.as_str(),
            value["moduleId"].as_str().unwrap()
        );
        assert_eq!(input.stylesheet_imports, ["virtual:styles"]);
        assert_eq!(
            input.module_css_import.as_deref(),
            Some("virtual:module-css")
        );
        assert_eq!(input.framework, Some(AdapterKind::Svelte));
        assert_eq!(input.syntax().language(), ScriptLanguage::Ts);
        if let Some(projection) = input.projection() {
            assert_eq!(input.syntax().module_kind(), ScriptModuleKind::Module);
            assert_eq!(
                input.parser_source().text(),
                value["projection"]["code"].as_str().unwrap()
            );
            assert!(input.parser_source().uri().is_none());
            assert!(input.parser_source().source_map_source().is_none());
            let parser_id = input.parser_source().id().0;
            assert_eq!(input.document().file().id().0, parser_id + 1);
            assert_eq!(projection.analysis_id().0, parser_id + 2);
            assert_eq!(
                input.document().file().uri().unwrap().as_str(),
                "test:///entry.svelte"
            );
            assert_eq!(
                input.document().file().source_map_source(),
                Some("entry.svelte")
            );
            assert_eq!(
                input.document().file().text(),
                value["document"]["code"].as_str().unwrap()
            );
            assert_eq!(projection.import_realms()[0].source.start.0, 12);
            assert_eq!(projection.import_realms()[0].source.end.0, 28);
            assert_eq!(
                projection.editable_regions()[0].source,
                projection.import_realms()[0].source
            );
        } else {
            assert_eq!(input.syntax().module_kind(), ScriptModuleKind::CommonJs);
            assert_eq!(
                input.parser_source().uri().unwrap().as_str(),
                "test:///entry.ts"
            );
            assert_eq!(input.parser_source().source_map_source(), Some("entry.ts"));
            assert_eq!(
                input.parser_source().text(),
                value["document"]["code"].as_str().unwrap()
            );
        }
    }
}

#[test]
fn prepared_projection_rejects_surrogate_interiors_and_invalid_ranges() {
    for field in ["importRealms", "editableRegions"] {
        for (start, end) in [(1, 2), (26, 10), (10, 100)] {
            let mut value = projected_prepared_value();
            value["projection"][field][0]["start"] = start.into();
            value["projection"][field][0]["end"] = end.into();
            let source: HostPreparedSource = serde_json::from_value(value).unwrap();
            assert!(
                source.input(&mut SourceRegistry::default()).is_err(),
                "{field}: {start}..{end}"
            );
        }
    }
}

#[test]
fn host_projection_forwards_core_metadata_errors() {
    for (field, replacement, message) in [
        (
            "importRealms",
            serde_json::json!([]),
            "editable region 0 references missing import realm 0",
        ),
        (
            "editableRegions",
            serde_json::json!([{ "importRealm": 99, "start": 10, "end": 26 }]),
            "editable region 0 references missing import realm 99",
        ),
        (
            "converter",
            serde_json::json!({ "name": "", "version": "1", "optionsFingerprint": "test" }),
            "converter identity field name must not be blank",
        ),
    ] {
        let mut value = projected_prepared_value();
        value["projection"][field] = replacement;
        let source: HostPreparedSource = serde_json::from_value(value).unwrap();
        let error = source.input(&mut SourceRegistry::default()).unwrap_err();
        assert_eq!(error.reason, message);
    }
    let mut value = projected_prepared_value();
    value["projection"]["editableRegions"] = serde_json::json!([]);
    let source: HostPreparedSource = serde_json::from_value(value).unwrap();
    assert!(source.input(&mut SourceRegistry::default()).is_ok());
}
