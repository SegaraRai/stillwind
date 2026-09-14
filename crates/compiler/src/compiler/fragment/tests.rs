//! Folding proofs and differential execution against the fragment runtime.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use stillwind_ir::{
    AdapterKind, ConditionalKind, FRAMEWORKS, FrameworkSpec, ModuleAnalysis, SourceAnalyzer,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile, Span};

use super::expression::{Atom, Emitted, EmittedEntry, Fragment, Property, ValueExpr};
use super::fold::{Demand, Rejection, fold, record_fields};
use super::render::{render_reference, render_style_call};
use crate::compiler::props::{StyleEmission, StyleEmissionPart};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// Builds an effectful value call with a synthetic span used only as occurrence identity.
/// The short span is not an exact text mapping for `render_transforms`.
fn property(name: &str, index: u32) -> Property {
    Property {
        pure: false,
        name: name.into(),
        value: format!("value({index})"),
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
        static_names: if base { vec!["_base".into()] } else { vec![] },
        parts,
        classes: vec![],
        call_span: Span::new(FileId(1), 0, 50),
    }
}

/// Uses identical value text at distinct spans to pin occurrence-based order rejection.
#[test]
fn record_proof_uses_occurrences_not_identical_expression_text() {
    let mut a = property("--x", 1);
    let mut b = property("data-x", 2);
    let mut c = property("--y", 3);
    a.value = "same()".into();
    b.value = a.value.clone();
    c.value = a.value.clone();
    let atoms = vec![Atom::Custom(a), Atom::Attribute(b), Atom::Custom(c)];
    assert_eq!(
        record_fields(&atoms, Demand::Props, AdapterKind::React.spec()),
        Err(Rejection::EvaluationOrder {
            before: 0,
            between: 1,
            after: 2
        })
    );
}

/// Checks that duplicate custom names share an object channel but block direct string serialization.
#[test]
fn duplicate_custom_names_require_object_representation() {
    let atoms = vec![
        Atom::Custom(property("--x", 1)),
        Atom::Custom(property("--x", 2)),
    ];
    assert!(record_fields(&atoms, Demand::Props, AdapterKind::React.spec()).is_ok());
    assert_eq!(
        record_fields(&atoms, Demand::Props, AdapterKind::Svelte.spec()),
        Err(Rejection::Representation)
    );
    assert!(record_fields(&atoms, Demand::Fragment, AdapterKind::Svelte.spec()).is_ok());
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
        Err(Rejection::Representation)
    );
    assert!(record_fields(&atoms, Demand::Fragment, AdapterKind::React.spec()).is_ok());
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
            .map(|i| Fragment::Record(vec![Atom::Custom(property(&format!("--v{i}"), i))]))
            .collect(),
    );
    let folded = fold(source, AdapterKind::React.spec());
    assert!(matches!(&folded, Fragment::Record(atoms) if atoms.len() == 4096));
    assert_eq!(fold(folded.clone(), AdapterKind::React.spec()), folded);
}

/// Enumerates effect masks across alternating channels and checks every surviving record proof.
#[test]
fn a_later_effect_cannot_join_an_already_reordered_pure_record() {
    for effects in 0..16 {
        let atoms = (0..4)
            .map(|i| {
                let mut p = property(if i % 2 == 0 { "--x" } else { "data-value" }, i);
                p.pure = effects & (1 << i) == 0;
                if i % 2 == 0 {
                    Atom::Custom(p)
                } else {
                    Atom::Attribute(p)
                }
            })
            .collect::<Vec<_>>();
        let source = Fragment::Sequence(
            atoms
                .into_iter()
                .map(|atom| Fragment::Record(vec![atom]))
                .collect(),
        );
        let fw = AdapterKind::React.spec();
        let folded = fold(source, fw);
        let records = match &folded {
            Fragment::Sequence(children) => children.as_slice(),
            record => std::slice::from_ref(record),
        };
        for record in records {
            let Fragment::Record(atoms) = record else {
                panic!("expected record")
            };
            assert!(
                record_fields(atoms, Demand::Fragment, fw).is_ok(),
                "{effects}: {atoms:?}"
            );
        }
        assert_eq!(fold(folded.clone(), fw), folded);
    }
}

/// Enumerates channel, effect, branch, and opaque-value combinations for every framework runtime.
#[test]
#[allow(clippy::too_many_lines)] // The matrix lists independent channel and control-flow cases.
fn folded_javascript_matches_runtime_materialization() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "fold.tsx",
            "import { style } from 'stillwind/react'; style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
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
        // Pure reads share state with effectful calls, exposing illegal read/write motion.
        for effects in 0..8 {
            for mask in 0..64u32 {
                let parts = (0..3)
                    .map(|i| {
                        let mut part = match (mask >> (2 * i)) & 3 {
                            0 => custom("--x", i + 1),
                            1 => custom("--y", i + 1),
                            2 => attribute("data-value", i + 1),
                            _ => attribute("style", i + 1),
                        };
                        if effects & (1 << i) == 0
                            && let StyleEmissionPart::CustomProperties { property, .. }
                            | StyleEmissionPart::Attributes { property, .. } = &mut part
                        {
                            property.pure = true;
                            property.value = format!("state.current + {i}");
                        }
                        part
                    })
                    .collect();
                append_case(&mut cases, &emission(parts, mask % 2 == 0), fw, &analysis);
            }
        }
        for kind in [ConditionalKind::LogicalAnd, ConditionalKind::Ternary] {
            for base in [false, true] {
                let branch = Emitted::Conditional {
                    pure: false,
                    code: "test()".into(),
                    kind,
                    consequent: Some(Box::new(Emitted::Group(vec![
                        Some(EmittedEntry::CustomProperty(property("--x", 2))),
                        Some(EmittedEntry::Attribute(property("data-state", 3))),
                    ]))),
                    alternate: Some(Box::new(Emitted::Group(vec![Some(
                        EmittedEntry::CustomProperty(property("--y", 4)),
                    )]))),
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
                    pure: false,
                    code: "test()".into(),
                    kind,
                    consequent: Some(Box::new(Emitted::Leaf {
                        names: vec!["_branch".into()],
                    })),
                    alternate: None,
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
                            emitted: Emitted::Expression("value(1)".into()),
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
                pure: false,
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
            StyleEmissionPart::Style {
                expression: "payload()".into(),
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
    execute_cases(&cases);
}

/// Runs optimized/reference pairs against framework runtimes in fresh JavaScript environments.
///
/// # Panics
///
/// Panics if Node or the repository test script is unavailable, process I/O fails, or
/// any value, evaluation trace, exception, or property-order comparison differs.
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
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Appends one optimized/reference pair, allocating independent helper bindings for each.
/// `analysis` supplies the module naming context; `fw` selects materialization semantics.
fn append_case(
    cases: &mut Vec<serde_json::Value>,
    input: &StyleEmission,
    fw: &FrameworkSpec,
    analysis: &ModuleAnalysis,
) {
    let optimized = render_style_call(
        input,
        fw,
        &mut RuntimeBindings::new(analysis),
        &mut Vec::new(),
    );
    let reference = render_reference(input, fw, &mut RuntimeBindings::new(analysis));
    cases.push(serde_json::json!({
        "framework": fw.macro_import.trim_start_matches("stillwind/"),
        "optimized": optimized.text,
        "reference": reference.text,
    }));
}
