//! Checks prepared document transport and projection contracts.

use super::*;

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
    let runtime = "import { stillwindClsx } from \"stillwind/runtime/svelte\";";
    assert!(code.starts_with(&format!("{prefix}{runtime}")), "{code}");
    assert!(
        code.contains(&format!(
            "{middle}import {{ stillwindClsx as stillwindClsx_1 }}"
        )),
        "{code}"
    );
    assert!(code.ends_with(suffix));
    assert_eq!(code.matches("stillwind/runtime/svelte").count(), 2);
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
        assert_eq!(source.type_facts(), []);
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
