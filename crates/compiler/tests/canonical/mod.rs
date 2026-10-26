//! Tests that canonical spellings match the pinned results of Tailwind's
//! canonicalization.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use stillwind_compiler::{CompilerOptions, DesignSystemQuery, RootFontSize};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CanonicalUtilitiesOverride, LintOverrides};
use stillwind_source::{FileId, SourceFile};
use stillwind_test_support::expand_css;

mod policy;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    name: String,
    css: String,
    base_theme: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    input: Vec<String>,
    expected: Vec<String>,
    #[serde(default, rename = "stillwindExpected")]
    stillwind_expected: Option<Vec<String>>,
    #[serde(default)]
    options: Options,
    #[serde(default)]
    contains: bool,
}

#[derive(Default, Deserialize)]
struct Options {
    rem: Option<f64>,
    #[serde(default)]
    collapse: bool,
    #[serde(default, rename = "logicalToPhysical")]
    logical_to_physical: bool,
}

#[test]
fn matches_pinned_canonical_contracts() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/canonical");
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    let mut failures = Vec::new();
    for path in paths {
        let fixture: Fixture = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let snapshot = snapshot(&fixture);
        for case in fixture.cases {
            let expected = case.stillwind_expected.as_ref().unwrap_or(&case.expected);
            let query = DesignSystemQuery::new(
                snapshot.clone(),
                CompilerOptions {
                    root_font_size: RootFontSize::new(case.options.rem.unwrap_or(16.0)).unwrap(),
                    lint: LintOverrides {
                        prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                            px_to_rem: Some(case.options.rem.is_some()),
                            collapse: Some(case.options.collapse),
                            logical_to_physical: Some(case.options.logical_to_physical),
                            preserve_specificity: Some(false),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            for (strategy, prefix, suffix) in [
                ("default", "", ""),
                ("variant", "focus:", ""),
                ("important", "", "!"),
            ] {
                let expected: Vec<_> = expected
                    .iter()
                    .map(|value| format!("{prefix}{value}{suffix}"))
                    .collect();
                let inputs: Vec<_> = case
                    .input
                    .iter()
                    .map(|value| format!("{prefix}{value}{suffix}").into())
                    .collect();
                let actual: Vec<_> = query
                    .canonical_candidates(&inputs)
                    .unwrap()
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect();
                let matches = if case.contains {
                    expected.iter().all(|expected| actual.contains(expected))
                } else {
                    actual == expected
                };
                let canonical_inputs: Vec<_> =
                    actual.iter().map(|value| value.as_str().into()).collect();
                let repeated: Vec<_> = query
                    .canonical_candidates(&canonical_inputs)
                    .unwrap()
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect();
                if repeated != actual {
                    failures.push(format!(
                        "{} is not stable: {:?} -> {:?}",
                        fixture.name, actual, repeated
                    ));
                }
                if !matches {
                    failures.push(format!(
                        "{} ({strategy}) {:?}: {:?} != {:?}",
                        fixture.name, case.input, actual, expected
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn snapshot(fixture: &Fixture) -> DesignSystemSnapshot {
    let imports = "@import 'stillwind/builtins.css'; @import 'stillwind/utilities.css';";
    let css = fixture
        .css
        .replace(
            "@import 'tailwindcss';",
            &format!("@import 'stillwind'; {}", fixture.base_theme),
        )
        .replace("@tailwind utilities;", imports)
        .replace("@plugin \"./plugin.js\";", "@stillwind-dynamic myicon-*;");
    let source = SourceFile::new(FileId(1), None, None, expand_css(&css)).unwrap();
    DesignSystemSnapshot::from_source(&source, "theme.css".into(), DesignSystemOptions::default())
        .unwrap()
}
