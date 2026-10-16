//! Findings of stylesheet lint at CSS ranges and the CSS that their fixes produce.

use std::sync::Arc;

use stillwind_design_system::{
    CssFileInput, DesignSystemLoadStep, DesignSystemLoader, DesignSystemSnapshot,
};
use stillwind_diagnostics::{LintKind, LintOverrides, LintSeverity};
use stillwind_source::{SourceRegistry, SourceUri};
use stillwind_test_support::expand_css;

use super::lint_stylesheets;
use crate::CompilerOptions;

/// Text that the user source starts with before the CSS under test.
const IMPORT: &str = "@import 'stillwind';\n";

/// Loads `css` as a marked user source that imports the unmarked package.
pub(super) fn snapshot(css: &str, options: &CompilerOptions) -> DesignSystemSnapshot {
    let file = |id, name: &str, uri: &str, content: String, lint| CssFileInput {
        id,
        name: name.into(),
        uri: Some(SourceUri::parse(uri).unwrap()),
        source_map_source: None,
        content: content.into(),
        lint,
    };
    let mut loader = DesignSystemLoader::new(options.design_system(), SourceRegistry::default());
    let DesignSystemLoadStep::NeedFile(requests) = loader.provide_many([file(
        0,
        "app.css",
        "file:///project/app.css",
        format!("{IMPORT}{css}"),
        true,
    )]) else {
        panic!("the user source imports the package");
    };
    let step = loader.provide_many([file(
        requests[0].id,
        "stillwind",
        "file:///project/node_modules/stillwind/index.css",
        expand_css(IMPORT),
        false,
    )]);
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = step else {
        panic!("{step:?}");
    };
    snapshot
}

/// Returns the rule, covered text, and fixed CSS of each finding for `css`.
pub(super) fn findings(
    css: &str,
    options: &CompilerOptions,
) -> Vec<(LintKind, String, Option<String>)> {
    let snapshot = snapshot(css, options);
    let source = Arc::clone(&snapshot.sources().sources[0].source);
    lint_stylesheets(&snapshot, options)
        .findings
        .iter()
        .map(|finding| {
            let fixed = finding.actions.first().map(|action| {
                let mut text = source.text().to_owned();
                for edit in action.edits.iter().rev() {
                    text.replace_range(
                        edit.range.start() as usize..edit.range.end() as usize,
                        &edit.replacement,
                    );
                }
                text[IMPORT.len()..].to_owned()
            });
            (
                finding.kind(),
                source.slice(finding.labels[0].span).unwrap().to_owned(),
                fixed,
            )
        })
        .collect()
}

/// Returns [`findings`] under the default options.
pub(super) fn default_findings(css: &str) -> Vec<(LintKind, String, Option<String>)> {
    findings(css, &CompilerOptions::default())
}

pub(super) fn finding(kind: LintKind, covered: &str) -> (LintKind, String, Option<String>) {
    (kind, covered.to_owned(), None)
}

pub(super) fn fixed(
    kind: LintKind,
    covered: &str,
    css: &str,
) -> (LintKind, String, Option<String>) {
    (kind, covered.to_owned(), Some(css.to_owned()))
}

#[test]
fn unresolved_candidates_and_variants_are_reported_at_their_text() {
    assert_eq!(
        default_findings(".btn { @apply unknown-x flex hover:nope:flex; }"),
        [
            finding(LintKind::UnknownUtility, "unknown-x"),
            finding(LintKind::UnknownVariant, "nope"),
        ]
    );
    assert_eq!(
        default_findings(".card { @variant hover:nope, print, { color: red; } }"),
        [
            finding(LintKind::UnknownVariant, "nope"),
            finding(LintKind::InvalidVariant, ""),
        ]
    );
}

#[test]
fn package_css_and_definitions_are_not_checked() {
    assert_eq!(
        default_findings("@utility thing { @apply unknown-x; } @variant nope { .x { } }"),
        []
    );
}

#[test]
fn application_variants_are_checked() {
    assert_eq!(
        default_findings(".a { @apply hover:hover:flex; }"),
        [fixed(
            LintKind::RedundantVariant,
            "hover",
            ".a { @apply hover:flex; }"
        )]
    );
}

#[test]
fn nested_variant_blocks_form_one_condition_stack() {
    assert_eq!(
        default_findings(".a { @variant hover { @variant focus:hover { color: red; } } }"),
        [fixed(
            LintKind::RedundantVariant,
            "hover",
            ".a { @variant hover { @variant focus { color: red; } } }"
        )]
    );
    assert_eq!(
        default_findings(".a { @variant hover { @apply hover:flex; } }"),
        [fixed(
            LintKind::RedundantVariant,
            "hover",
            ".a { @variant hover { @apply flex; } }"
        )]
    );
    assert_eq!(
        default_findings(".a { @variant hover { .b { @apply hover:flex; } } }"),
        [],
        "a style rule selects a new element"
    );
}

#[test]
fn candidates_of_one_application_are_compared() {
    let found = default_findings(".a { @apply flex flex block; } .b { @apply flex; }");
    assert_eq!(
        found
            .iter()
            .map(|(kind, covered, _)| (*kind, covered.as_str()))
            .collect::<Vec<_>>(),
        [
            (LintKind::DuplicateUtility, "flex"),
            (LintKind::ConflictingUtilities, "block"),
            (LintKind::ConflictingUtilities, "block"),
        ],
        "each earlier `flex` conflicts with `block`"
    );
}

#[test]
fn selector_variants_are_checked_against_the_rule_selector() {
    assert_eq!(
        default_findings(".a { @apply [:root]:flex; @variant [:root] { color: red; } }"),
        [
            finding(LintKind::DocumentScopedSelector, "[:root]"),
            finding(LintKind::DocumentScopedSelector, "[:root]"),
        ]
    );
}

#[test]
fn fixes_edit_the_stylesheet() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_sorted_utilities: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert_eq!(
        findings(".a { @apply p-4 /* x */ [display:flex]; }", &options),
        [
            fixed(
                LintKind::PreferSortedUtilities,
                "p-4 /* x */ [display:flex]",
                ".a { @apply [display:flex] /* x */ p-4; }"
            ),
            fixed(
                LintKind::PreferCanonicalUtilities,
                "[display:flex]",
                ".a { @apply p-4 /* x */ flex; }"
            ),
        ]
    );
}

#[test]
fn suppression_comments_use_stylesheet_syntax() {
    assert_eq!(
        default_findings(
            ".a {\n  /* swlint-disable-next-line unknown-utility -- reason */\n  @apply unknown-x;\n}"
        ),
        []
    );
    // A backtick would start a template literal in a script and hide the comment.
    assert_eq!(
        default_findings(
            ".a {
  --tick: `;
  /* swlint-disable-next-line unknown-utility */
  @apply unknown-x;
}"
        ),
        []
    );
}

#[test]
fn one_analysis_is_shared_per_snapshot_and_options() {
    let options = CompilerOptions::default();
    let snapshot = snapshot(".a { @apply unknown-x; }", &options);
    assert!(Arc::ptr_eq(
        &lint_stylesheets(&snapshot, &options),
        &lint_stylesheets(&snapshot.clone(), &options)
    ));
}

#[test]
fn value_and_condition_rules_report_inside_the_candidate() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_typed_variables: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert_eq!(
        findings(
            ".a { @apply bg-red-500/[150%] opacity-[150%]; }\n\
             .b { @variant motion-safe { @apply motion-reduce:flex; } }\n\
             .c { @apply bg-(--brand); }",
            &options
        ),
        [
            finding(LintKind::InvalidPropertyValue, "[150%]"),
            finding(LintKind::ClampedValue, "[150%]"),
            finding(LintKind::UnsatisfiableVariants, "motion-reduce"),
            fixed(
                LintKind::PreferTypedVariables,
                "(--brand)",
                ".a { @apply bg-red-500/[150%] opacity-[150%]; }\n\
                 .b { @variant motion-safe { @apply motion-reduce:flex; } }\n\
                 .c { @apply bg-(color:--brand); }"
            ),
        ]
    );
}
