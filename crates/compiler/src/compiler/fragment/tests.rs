//! Folding proofs and differential execution against the fragment runtime.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use lean_string::{LeanString, ToLeanStr, ToLeanString};

use stillwind_diagnostics::LintSettings;
use stillwind_ir::{
    AdapterKind, ConditionalBranches, FRAMEWORKS, FrameworkSpec, ModuleAnalysis, ScriptLanguage,
    ScriptModuleKind, SourceAnalyzer,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{FileId, ModuleId, SourceFile, Span};

use super::expression::{Atom, Emitted, EmittedEntry, Fragment, Property, ValueExpr};
use super::fold::{Demand, fold, record_fields};
use super::render::{render_reference, render_style_call};
use crate::IdentifierMode;
use crate::compiler::expression_render::render_plain;
use crate::compiler::props::{StyleEmission, StyleEmissionPart};
use crate::runtime::{RuntimeBindings, RuntimeHelper};
use crate::style_analysis::{StyleEffect, StyleEffects};

/// Returns the selected operator's branch shape for both differential test cases. The
/// alternate fixture is unused for logical conjunction.
fn branches<T>(ternary: bool, consequent: T, alternate: T) -> ConditionalBranches<T> {
    if ternary {
        ConditionalBranches::Ternary {
            consequent,
            alternate,
        }
    } else {
        ConditionalBranches::And { consequent }
    }
}

/// Builds a value call with a synthetic span used only as occurrence identity.
/// The short span is not an exact text mapping for `render_transforms`.
fn property(name: &str, index: u32) -> Property {
    Property {
        static_value: None,
        computed_value: None,
        name: name.into(),
        value: format_args!("value({index})").to_lean_str(),
        span: Span::new(FileId(1), index, index + 1),
    }
}

/// Builds a custom-property contribution whose ordering position matches its synthetic identity.
fn custom(name: &str, index: u32) -> StyleEmissionPart {
    StyleEmissionPart::CustomProperties {
        property: property(name, index),
        position: index,
    }
}

/// Builds an ordinary attribute contribution whose value evaluation is observable in the harness.
fn attribute(name: &str, index: u32) -> StyleEmissionPart {
    StyleEmissionPart::Attributes {
        property: property(name, index),
        position: index,
    }
}

/// Builds a finalized call fixture with an optional static class and no allocated CSS records.
fn emission(parts: Vec<StyleEmissionPart>, base: bool) -> StyleEmission {
    StyleEmission {
        style_effects: StyleEffects {
            lint_settings: LintSettings::with_defaults(),
            identity: IdentityId::new(AssetKind::Style, ModuleId::from("test"), 0, 0, ""),
            span: Span::new(FileId(1), 0, 0),
            root: StyleEffect::All(Vec::new()),
        },
        static_names: if base { vec!["_base".into()] } else { vec![] },
        parts,
        classes: vec![],
        call_span: Span::new(FileId(1), 0, 50),
        written_name_symbols: Vec::new(),
    }
}

#[test]
fn record_layout_combines_interleaved_calls_under_the_contract() {
    let atoms = vec![
        Atom::Custom(property("--x", 1)),
        Atom::Attribute(property("data-x", 2)),
        Atom::Custom(property("--y", 3)),
    ];
    assert!(record_fields(&atoms, Demand::Props, AdapterKind::React.spec()).is_some());
}

/// Checks that duplicate custom names share an object channel but block direct string serialization.
#[test]
fn duplicate_custom_names_require_object_representation() {
    let atoms = vec![
        Atom::Custom(property("--x", 1)),
        Atom::Custom(property("--x", 2)),
    ];
    assert!(record_fields(&atoms, Demand::Props, AdapterKind::React.spec()).is_some());
    assert_eq!(
        record_fields(&atoms, Demand::Props, AdapterKind::Svelte.spec()),
        None
    );
    assert!(record_fields(&atoms, Demand::Fragment, AdapterKind::Svelte.spec()).is_some());
}

/// Checks the overlay boundary where an ordinary style value coexists with custom properties.
#[test]
fn record_channels_do_not_confuse_ordinary_style_with_custom_properties() {
    let atoms = vec![
        Atom::Attribute(property("style", 1)),
        Atom::Custom(property("--x", 2)),
    ];
    assert_eq!(
        record_fields(&atoms, Demand::Props, AdapterKind::React.spec()),
        None
    );
    assert!(record_fields(&atoms, Demand::Fragment, AdapterKind::React.spec()).is_some());
}

/// Keeps a base props conversion outside a neighboring folded custom-property record.
#[test]
fn partial_folding_stops_at_opaque_props_and_is_idempotent() {
    let source = Fragment::from_emission(&emission(
        vec![
            StyleEmissionPart::Base {
                expression: "base".into(),
                position: 0,
            },
            custom("--x", 1),
            custom("--y", 2),
        ],
        false,
    ));
    let folded = fold(source, AdapterKind::Svelte.spec());
    assert!(
        matches!(&folded, Fragment::Sequence(children) if matches!(&children[..], [Fragment::FromProps(_), Fragment::Record(atoms)] if atoms.len() == 2))
    );
    assert_eq!(fold(folded.clone(), AdapterKind::Svelte.spec()), folded);
}

/// Exercises a 4096-entry record and verifies a second fold leaves its structure unchanged.
#[test]
fn large_records_fold_without_candidate_growth() {
    let source = Fragment::Sequence(
        (0..4096)
            .map(|i| {
                Fragment::Record(vec![Atom::Custom(property(
                    &format_args!("--v{i}").to_lean_string(),
                    i,
                ))])
            })
            .collect(),
    );
    let folded = fold(source, AdapterKind::React.spec());
    assert!(matches!(&folded, Fragment::Record(atoms) if atoms.len() == 4096));
    assert_eq!(fold(folded.clone(), AdapterKind::React.spec()), folded);
}

/// Enumerates channel, branch, and opaque-value combinations for every framework runtime.
#[test]
#[allow(clippy::too_many_lines)] // The matrix lists independent channel and control-flow cases.
fn folded_javascript_matches_runtime_materialization() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let mut cases = Vec::new();
    // Exhaust all three-contribution channel combinations, including overwrites.
    for fw in FRAMEWORKS {
        for mask in 0..64u32 {
            let parts = (0..3)
                .map(|i| match (mask >> (2 * i)) & 3 {
                    0 => custom("--x", i + 1),
                    1 => custom("--y", i + 1),
                    2 => attribute("data-value", i + 1),
                    _ => attribute("style", i + 1),
                })
                .collect();
            let input = emission(parts, mask % 2 == 0);
            append_case(&mut cases, &input, fw, &analysis);
        }
        for ternary in [false, true] {
            for base in [false, true] {
                let branch = Emitted::Conditional {
                    code: "test()".into(),
                    branches: branches(
                        ternary,
                        Some(Box::new(Emitted::Group(vec![
                            Some(EmittedEntry::CustomProperty(property("--x", 2))),
                            Some(EmittedEntry::Attribute(property("data-state", 3))),
                        ]))),
                        Some(Box::new(Emitted::Group(vec![Some(
                            EmittedEntry::CustomProperty(property("--y", 4)),
                        )]))),
                    ),
                };
                append_case(
                    &mut cases,
                    &emission(
                        vec![
                            custom("--before", 1),
                            StyleEmissionPart::Class {
                                emitted: branch,
                                position: 2,
                            },
                            custom("--after", 5),
                        ],
                        base,
                    ),
                    fw,
                    &analysis,
                );
                let class = Emitted::Conditional {
                    code: "test()".into(),
                    branches: branches(
                        ternary,
                        Some(Box::new(Emitted::Leaf {
                            names: vec!["_branch".into()],
                        })),
                        None,
                    ),
                };
                append_case(
                    &mut cases,
                    &emission(
                        vec![
                            StyleEmissionPart::Class {
                                emitted: class,
                                position: 1,
                            },
                            custom("--x", 2),
                        ],
                        base,
                    ),
                    fw,
                    &analysis,
                );
            }
        }
        for base in [false, true] {
            append_case(
                &mut cases,
                &emission(
                    vec![
                        StyleEmissionPart::Class {
                            emitted: Emitted::Expression("String(value(1))".into()),
                            position: 1,
                        },
                        custom("--x", 2),
                    ],
                    base,
                ),
                fw,
                &analysis,
            );
        }
        for base in [false, true] {
            let external = Emitted::Expression(ValueExpr::Call {
                helper: RuntimeHelper::Clsx,
                arguments: vec!["value(1)".into()],
                span: Span::new(FileId(1), 1, 2),
            });
            append_case(
                &mut cases,
                &emission(
                    vec![
                        StyleEmissionPart::Class {
                            emitted: external,
                            position: 1,
                        },
                        attribute("data-after", 2),
                        custom("--x", 3),
                    ],
                    base,
                ),
                fw,
                &analysis,
            );
        }
        for opaque in [
            StyleEmissionPart::Base {
                expression: "base()".into(),
                position: 1,
            },
            StyleEmissionPart::Class {
                emitted: Emitted::Group(vec![Some(EmittedEntry::StylePayload("payload()".into()))]),
                position: 1,
            },
        ] {
            append_case(
                &mut cases,
                &emission(vec![opaque, custom("--x", 2), custom("--y", 3)], true),
                fw,
                &analysis,
            );
        }
    }
    // Compare against unflattened class values independently of the fragment renderer.
    for scenario in 0..4 {
        let calls = (1..=3)
            .map(|index| {
                Some(Emitted::Expression(ValueExpr::Call {
                    helper: RuntimeHelper::Clsx,
                    arguments: vec![format_args!("value({index})").to_lean_string().into()],
                    span: Span::new(FileId(1), index, index + 1),
                }))
            })
            .collect::<Vec<_>>();
        let value = Emitted::Array(vec![None, Some(Emitted::Array(calls))]);
        let value = if scenario >= 2 {
            let guard = Emitted::Conditional {
                code: "state.current".into(),
                branches: ConditionalBranches::And {
                    consequent: Some(Box::new(Emitted::Leaf {
                        names: vec!["active".into()],
                    })),
                },
            };
            let call = Emitted::Expression(ValueExpr::Call {
                helper: RuntimeHelper::Clsx,
                arguments: vec!["value(2)".into()],
                span: Span::new(FileId(1), 2, 3),
            });
            Emitted::Array(if scenario == 2 {
                vec![Some(guard), Some(call)]
            } else {
                vec![Some(call), Some(guard)]
            })
        } else if scenario == 1 {
            Emitted::Conditional {
                code: "test()".into(),
                branches: ConditionalBranches::And {
                    consequent: Some(Box::new(value)),
                },
            }
        } else {
            value
        };
        let input = emission(
            vec![StyleEmissionPart::Class {
                emitted: value.clone(),
                position: 1,
            }],
            false,
        );
        let optimized = render_style_call(
            &input,
            AdapterKind::React.spec(),
            &mut RuntimeBindings::new(&analysis),
        );
        let reference = render_plain(&value, &mut RuntimeBindings::new(&analysis)).unwrap();
        cases.push(serde_json::json!({
            "framework": "react",
            "optimized": optimized.text,
            "reference": format_args!("stillwindMerge({{[stillwindClassKey]:stillwindClsx({})}})", reference.text).to_lean_string(),
        }));
    }
    execute_cases(&cases);
}

/// Runs optimized/reference pairs against framework runtimes in fresh JavaScript environments.
///
/// # Panics
///
/// Panics if Node or the repository test script is unavailable, process I/O fails, or
/// any value, runtime error, or property-order comparison differs.
fn execute_cases(cases: &[serde_json::Value]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let script = root.join("crates/compiler/tests/fragment-execution.mjs");
    let mut child = Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node is required for generated-JS contract tests");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    println!("{}", LeanString::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}\n{}",
        LeanString::from_utf8_lossy(&output.stdout),
        LeanString::from_utf8_lossy(&output.stderr)
    );
}

/// Appends one optimized/reference pair, allocating independent helper bindings for
/// each. `analysis` supplies the module naming context; `fw` selects materialization
/// semantics.
fn append_case(
    cases: &mut Vec<serde_json::Value>,
    input: &StyleEmission,
    fw: &FrameworkSpec,
    analysis: &ModuleAnalysis,
) {
    let optimized = render_style_call(input, fw, &mut RuntimeBindings::new(analysis));
    let reference = render_reference(input, fw, &mut RuntimeBindings::new(analysis));
    cases.push(serde_json::json!({
        "framework": fw.macro_import.trim_start_matches("stillwind/"),
        "optimized": optimized.text,
        "reference": reference.text,
    }));
}

#[test]
fn class_branches_and_single_style_payloads_match_runtime_materialization() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let class = |name: &str| StyleEmissionPart::Class {
        emitted: Emitted::Leaf {
            names: vec![name.into()],
        },
        position: 1,
    };
    let mut cases = Vec::new();
    for fw in FRAMEWORKS {
        for base in [false, true] {
            for ternary in [false, true] {
                let parts = vec![StyleEmissionPart::Conditional {
                    test: "test()".into(),
                    branches: branches(ternary, vec![class("selected")], vec![class("other")]),
                    position: 1,
                }];
                let input = emission(parts, base);
                let fragment = Fragment::from_emission(&input);
                let folded = fold(fragment, fw);
                assert_eq!(fold(folded.clone(), fw), folded);
                append_case(&mut cases, &input, fw, &analysis);
            }
            for payload in [
                ValueExpr::Source("payload()".into()),
                ValueExpr::Entry {
                    property: "--x".into(),
                    value: Box::new(ValueExpr::Source("value(1)".into())),
                },
                ValueExpr::Entry {
                    property: "view-transition-name".into(),
                    value: Box::new(ValueExpr::Call {
                        helper: RuntimeHelper::ViewTransitionName,
                        arguments: vec!["'token'".into(), "'key'".into()],
                        span: Span::new(FileId(1), 1, 2),
                    }),
                },
            ] {
                let input = emission(
                    vec![StyleEmissionPart::Class {
                        emitted: Emitted::Group(vec![Some(EmittedEntry::StylePayload(payload))]),
                        position: 2,
                    }],
                    base,
                );
                append_case(&mut cases, &input, fw, &analysis);
            }
        }
    }
    execute_cases(&cases);
}

#[test]
fn style_fields_follow_framework_specification() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let original = AdapterKind::React.spec();
    let framework = FrameworkSpec {
        id: original.id,
        macro_import: original.macro_import,
        runtime_import: original.runtime_import,
        runtime_import_dev: original.runtime_import_dev,
        class_property: original.class_property,
        style_property: "inlineStyle",
        style_format: original.style_format,
        projected_input: original.projected_input,
        normalize: original.normalize,
    };
    for part in [
        custom("--x", 1),
        StyleEmissionPart::Class {
            emitted: Emitted::Group(vec![Some(EmittedEntry::StylePayload("payload()".into()))]),
            position: 1,
        },
    ] {
        let output = render_style_call(
            &emission(vec![part], false),
            &framework,
            &mut RuntimeBindings::new(&analysis),
        );
        assert!(output.text.contains("inlineStyle:"), "{}", output.text);
        assert!(!output.text.contains("style:"), "{}", output.text);
    }
}

#[test]
fn ref_reads_select_validation_by_compiler_mode_and_keep_source_evaluation_once() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let span = Span::new(FileId(1), 39, 45);
    let expression = ValueExpr::RefClassRead {
        value: "load()".into(),
        span,
    };
    for (mode, expected) in [
        (IdentifierMode::Release, "(load()).c"),
        (
            IdentifierMode::Development,
            "stillwindRefClassChecked(load())",
        ),
    ] {
        let mut runtime = RuntimeBindings::new(&analysis).in_mode(mode);
        assert_eq!(expression.render(&mut runtime).text, expected);
    }
    let lookup = ValueExpr::Lookup {
        value: Box::new(ValueExpr::Source("load()".into())),
        key: "c".to_lean_str(),
        table: "{marker:'class'}".into(),
        span,
    };
    for (mode, expected) in [
        (IdentifierMode::Release, "({marker:'class'})[(load()).c]"),
        (
            IdentifierMode::Development,
            "({marker:'class'})[stillwindRefClassChecked(load())]",
        ),
    ] {
        let mut runtime = RuntimeBindings::new(&analysis).in_mode(mode);
        assert_eq!(lookup.render(&mut runtime).text, expected);
    }
}
