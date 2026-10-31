//! Utility emission, precedence, registration dependencies, and diagnostics.

use stillwind_compiler::CompilerOptions;
use stillwind_css::MAX_VALUE_NESTING;
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintDiagnostic, LintOverrides, LintSeverity,
};

use super::compile;
use super::support::CompilationFixture;

#[test]
fn unresolved_utilities_are_omitted_independently_of_reporting_and_class_identity() {
    let css = "@utility a { color:red; } @variant hover { &:hover { @slot; } }";
    for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
        let options = CompilerOptions {
            lint: LintOverrides {
                unknown_utility: Some(severity),
                invalid_utility: Some(severity),
                ..LintOverrides::default()
            },
            ..CompilerOptions::default()
        };
        let fixture = CompilationFixture::new(css, &options);
        for (valid, input) in [
            ("'a'", "'a missing bad-['"),
            ("'a'", "['missing', 'a', 'bad-[']"),
            ("'hover:a'", "'hover:missing hover:a bad-['"),
            ("{'hover:': 'a'}", "{'hover:': 'missing a bad-['}"),
            ("active && 'a'", "active && 'missing a bad-['"),
            ("active ? 'a' : false", "active ? 'missing a bad-[' : false"),
            ("''", "'missing bad-['"),
            (
                "{class: 'external-widget'}, 'a'",
                "{class: 'external-widget'}, 'missing a bad-['",
            ),
        ] {
            let (_, expected) = fixture.compile(&format!("export const x = style({valid});"));
            let (_, result) = fixture.compile(&format!("export const x = style({input});"));
            assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
            assert_eq!(
                result
                    .lint_findings
                    .iter()
                    .filter(|finding| matches!(
                        finding.diagnostic,
                        LintDiagnostic::UnknownUtility { .. }
                            | LintDiagnostic::InvalidUtility { .. }
                    ))
                    .count(),
                if severity == LintSeverity::Off { 0 } else { 2 },
                "{input}: {result:#?}"
            );
            assert_eq!(result.css, expected.css, "{input}");
            assert_eq!(
                result
                    .transforms
                    .iter()
                    .map(|transform| &transform.replacement)
                    .collect::<Vec<_>>(),
                expected
                    .transforms
                    .iter()
                    .map(|transform| &transform.replacement)
                    .collect::<Vec<_>>(),
                "{input}"
            );
        }
    }
}

/// An arbitrary value that the property grammar rejects is emitted as written at every
/// reporting level, and only the finding depends on the severity. The finding covers
/// the value, and a property chosen as a fallback between value types gets hinted
/// spellings.
#[test]
fn invalid_property_values_are_emitted_independently_of_reporting() {
    let css = "@utility pad-* { padding: --value([length-percentage]); } @utility tone-* { color: --value([color]); } @utility tone-* { font-size: --value([length]); }";
    let code = "export const x = style('pad-[1/2] pad-[2px] [width:1/2] tone-[12px_foo]');";
    let expected = compile(code, css).1;
    for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
        let options = CompilerOptions {
            lint: LintOverrides {
                incompatible_property_value: Some(severity),
                ..LintOverrides::default()
            },
            ..CompilerOptions::default()
        };
        let (source, result) = CompilationFixture::new(css, &options).compile(code);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        for declaration in ["padding: 1/2;", "width:1/2;", "color: 12px foo;"] {
            assert!(result.css.contains(declaration), "{}", result.css);
        }
        assert_eq!(result.css, expected.css);
        let findings = result
            .lint_findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.diagnostic,
                    LintDiagnostic::IncompatiblePropertyValue { .. }
                )
            })
            .map(|finding| {
                (
                    source.slice(finding.labels[0].span).unwrap(),
                    finding.diagnostic.to_string(),
                    result.lint_settings.reporting_severity(finding.kind()),
                )
            })
            .collect::<Vec<_>>();
        if severity == LintSeverity::Off {
            assert_eq!(findings, [], "{result:#?}");
            continue;
        }
        assert_eq!(
            findings,
            [
                ("[1/2]", "`padding` does not accept `1/2`".into(), severity),
                ("1/2", "`width` does not accept `1/2`".into(), severity),
                (
                    "[12px_foo]",
                    "`tone-[12px_foo]` sets `color` because `12px foo` has no type hint and its type cannot be inferred, but `color` does not accept it; add a type hint, such as `tone-[color:12px_foo]` or `tone-[length:12px_foo]`".into(),
                    severity,
                ),
            ]
        );
    }
}

#[test]
fn deeply_nested_utility_values_fail_with_a_nesting_diagnostic() {
    let levels = MAX_VALUE_NESTING + 1;
    let value = format!("{}1{}", "(".repeat(levels), ")".repeat(levels));
    let utility = format!("w-[calc({value})]");
    let (source, result) = compile(
        &format!("export const x = style(\"{utility}\");"),
        "@theme { --spacing: 1px; }",
    );
    let diagnostic = result.diagnostics.first().expect("a nesting diagnostic");
    assert!(
        matches!(
            diagnostic.diagnostic,
            CompileDiagnostic::CssNestingTooDeep { limit, .. }
                if limit == MAX_VALUE_NESTING
        ),
        "{:#?}",
        result.diagnostics
    );
    assert_eq!(source.slice(diagnostic.labels[0].span).unwrap(), utility);
    assert!(result.css.is_empty(), "{}", result.css);
}

#[test]
fn utility_precedence_is_independent_of_literal_and_array_order() {
    let css = "@utility a { color:red; } @utility b { color:blue; } \
               @stillwind-utility builtin { color:green; }";
    for input in [
        "'a b builtin'",
        "'builtin b a'",
        "['b', 'builtin', 'a']",
        "'b', 'a', 'builtin'",
    ] {
        let (_, result) = compile(&format!("export const x = style({input});"), css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(
            result.lint_findings.iter().all(|finding| matches!(
                finding.diagnostic,
                LintDiagnostic::ConflictingUtilities { .. }
            )),
            "{result:#?}"
        );
        assert!(
            result.css.ends_with("{color:red;color:blue;color:green;}"),
            "{input}: {}",
            result.css
        );
    }
    for input in ["'hover:b hover:a'", "['hover:a', 'hover:b']"] {
        let (_, result) = compile(
            &format!("export const x = style({input});"),
            &format!("{css} @variant hover {{ &:hover {{ @slot; }} }}"),
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        let red = result.css.find("color:red;").unwrap();
        let blue = result.css.find("color:blue;").unwrap();
        assert!(red < blue, "{input}: {}", result.css);
    }
}

#[test]
fn utility_position_belongs_to_the_definition_name_across_undefinitions() {
    let replaced = "@stillwind-utility t { color:red; } @utility mid { color:blue; } \
                    @stillwind-undef-utility t;";
    let family = "@stillwind-utility t-* { color:red; } @utility mid { color:blue; } \
                  @stillwind-undef-utility t-*;";
    for (css, input, expected) in [
        (
            format!("{replaced} @utility t {{ color:green; }}"),
            "'mid t'",
            "{color:green;color:blue;}",
        ),
        (
            format!("{replaced} @stillwind-utility t {{ color:green; }}"),
            "'mid t'",
            "{color:green;color:blue;}",
        ),
        (
            format!("{replaced} @utility renamed {{ color:green; }}"),
            "'renamed mid'",
            "{color:blue;color:green;}",
        ),
        (
            format!("{family} @utility t-a {{ color:green; }}"),
            "'t-a mid'",
            "{color:blue;color:green;}",
        ),
    ] {
        let (_, result) = compile(&format!("export const x = style({input});"), &css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.css.ends_with(expected), "{css}: {}", result.css);
    }
}

#[test]
fn recursive_theme_macros_reject_the_candidate_without_partial_output() {
    for value in ["--value(--color-*)", "calc(--value(--color-*) + 1)"] {
        let (_, result) = compile(
            "const x = style('x-loop');",
            &format!(
                "@utility x-* {{ color: --value(--color-*); }} @theme inline {{ --color-loop: {value}; }}"
            ),
        );
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| matches!(finding.diagnostic, LintDiagnostic::UnknownUtility { .. })),
            "{result:#?}"
        );
        assert_eq!(result.css, "");
    }
}

#[test]
fn css_values_survive_emission_and_importance() {
    let (_, result) = compile(
        "const x = style('x!');",
        r#"@utility x { content: "calc(1 + 1);a"; width:calc(1px + 2px); background:url('data:text/plain;calc(1 + 1)'); }"#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"content: "calc(1 + 1);a"!important;"#),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("width:calc(1px + 2px)!important"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("data:text/plain;calc(1 + 1)"),
        "{}",
        result.css
    );
}

#[test]
fn mixed_variant_forms_override_in_source_order_even_on_one_line() {
    for (definitions, expected) in [
        (
            "@variant state { &:hover { @slot; } } @custom-variant state (&:focus);",
            ":focus",
        ),
        (
            "@custom-variant state (&:focus); @variant state { &:hover { @slot; } }",
            ":hover",
        ),
        (
            "@variant state { &:hover { @slot; } } @custom-variant state { &:active { @slot; } }",
            ":active",
        ),
    ] {
        let (_, result) = compile(
            "const x = style('state:x');",
            &format!("@utility x {{ color:red; }} {definitions}"),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.css.contains(expected), "{}", result.css);
    }
}

#[test]
fn literal_values_and_explicit_fallbacks_survive_utility_emission() {
    let (_, result) = compile(
        "const x = style('literal');",
        r#"
        @stillwind-property --registered { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility literal {
            content: ",var(--x) oklab(0% none none/0) deg*-1";
            transform: var(--component);
            width: var(--registered, 0px);
        }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"",var(--x) oklab(0% none none/0) deg*-1""#),
        "{}",
        result.css
    );
    assert!(result.css.contains("var(--component)"), "{}", result.css);
    assert!(!result.css.contains("var(--component,)"));
    assert!(
        result.css.contains("var(--registered, 0px)"),
        "{}",
        result.css
    );
}

#[test]
fn explicit_registration_dependencies_survive_apply_and_do_not_leak_into_css() {
    let (_, result) = compile(
        "const x = style('consumer');",
        r#"
        @stillwind-property --dependency { activation: dependency; syntax: "*"; inherits: false; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility consumer { @stillwind-require-properties --dependency; transform:var(--dependency,); }
        .authored { @apply consumer; }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("@property --dependency").count(), 1);
    assert!(!result.css.contains("@stillwind-require-properties"));
    let (_, missing) = compile(
        "const x = style('consumer');",
        "@utility consumer { @stillwind-require-properties --missing; color:red; }",
    );
    assert_ne!(missing.diagnostics, [] as [CompileError; 0]);
}

#[test]
fn variants_select_the_utility_element_before_the_utility_selector() {
    let (_, result) = compile(
        "const x = style('hover:child');",
        "@variant hover { &:hover { @slot; } } @utility child { & > span { color:red; } }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.css.contains(":hover > span{color:red"),
        "{}",
        result.css
    );
}

#[test]
fn utility_bodies_apply_other_utilities_under_their_variants() {
    let (_, result) = compile(
        "const x = style('hover:card');",
        "@variant hover { &:hover { @slot; } } @utility pad { padding:4px; } \
         @utility card { color:red; & > span { @apply pad; } }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.css.contains(":hover > span{padding:4px"),
        "{}",
        result.css
    );
    assert!(!result.css.contains("@apply"), "{}", result.css);
}

#[test]
fn custom_utilities_keep_sibling_selectors_and_expand_applications() {
    let (_, result) = compile(
        "const x = style('hover:framework-link');",
        r"@variant hover { &:hover { @slot; } }
@utility block { display:block; }
@utility underline { text-decoration-line:underline; }
@utility nav-link { text-underline-offset:5px; @apply hover:underline; &:focus-visible { outline:2px solid blue; } }
@utility framework-link {
  @apply block nav-link;
  [data-framework-icon] { color:gray; opacity:0.7; transition:color 140ms ease, opacity 140ms ease; }
  &:is(:hover, :focus-visible) [data-framework-icon] { color:black; opacity:1; }
}",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    let compact: String = result
        .css
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    for fragment in [
        ":hover{",
        "display:block",
        "text-underline-offset:5px",
        "&[data-framework-icon]{color:gray;opacity:0.7;transition:color140msease,opacity140msease;}",
        "&:is(:hover,:focus-visible)[data-framework-icon]{color:black;opacity:1;}",
        "&:focus-visible{outline:2pxsolidblue;}",
        "&:hover{text-decoration-line:underline;}",
    ] {
        assert!(compact.contains(fragment), "{}", result.css);
    }
    assert!(!result.css.contains("@apply"), "{}", result.css);
}

#[test]
fn custom_utility_rules_keep_declarations_and_conditions_in_source_order() {
    let (_, result) = compile(
        "const x = style('ordered');",
        r"@stillwind-format transform compose-space;
@utility ordered {
  transform:translateX(1px);
  color:red;
  & { color:blue; transform:none; }
  @media print { color:green; }
  color:black;
  transform:translateY(2px);
  [data-icon] { opacity:0.7; }
  color:purple;
}",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let positions: Vec<_> = [
        "transform:translateX(1px)",
        "color:red",
        "color:blue",
        "transform:none",
        "@media print{color:green",
        "color:black",
        "transform:translateY(2px)",
        "& [data-icon]{opacity:0.7",
        "color:purple",
    ]
    .map(|fragment| {
        result
            .css
            .find(fragment)
            .unwrap_or_else(|| panic!("{}", result.css))
    })
    .into();
    assert!(positions.is_sorted(), "{}", result.css);
}

#[test]
fn custom_utility_definitions_keep_each_nested_selector_in_registration_order() {
    let (_, result) = compile(
        "const x = style('nested');",
        r"@utility nested { &[data-a] { color:red; } &[data-b] { color:green; } }
@utility nested { &[data-a] { color:blue; } &[data-c] { color:purple; } }",
    );
    let positions = ["color:red", "color:green", "color:blue", "color:purple"].map(|fragment| {
        result
            .css
            .find(fragment)
            .unwrap_or_else(|| panic!("{}", result.css))
    });
    assert!(positions.is_sorted(), "{}", result.css);
}

#[test]
fn nested_utility_declarations_resolve_typed_branches_independently() {
    let css = r"@utility nested-* {
  display:block;
  [data-length] { width:--value([length]); }
  [data-color] { color:--value([color]); }
}";
    let (_, length) = compile("const x = style('nested-[2px]');", css);
    assert!(length.diagnostics.is_empty(), "{:?}", length.diagnostics);
    assert!(length.css.contains("display:block"), "{}", length.css);
    assert!(
        length.css.contains("& [data-length]{width:2px;"),
        "{}",
        length.css
    );
    assert!(!length.css.contains("[data-color]"), "{}", length.css);
    let (_, color) = compile("const x = style('nested-[red]');", css);
    assert!(
        color.css.contains("& [data-color]{color:red;"),
        "{}",
        color.css
    );
    assert!(!color.css.contains("[data-length]"), "{}", color.css);
    let (_, missing) = compile("const x = style('nested-missing');", css);
    assert!(!missing.css.contains("display:block"), "{}", missing.css);
}

#[test]
fn important_custom_utilities_prioritize_every_nested_declaration() {
    let (_, result) = compile(
        "const x = style('!nested');",
        "@utility nested { color:red; [data-icon] { opacity:0.7; } &:hover { color:blue; } }",
    );
    assert!(result.css.contains("color:red!important"), "{}", result.css);
    assert!(
        result.css.contains("opacity:0.7!important"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("color:blue!important"),
        "{}",
        result.css
    );
}

#[test]
fn utility_keyframes_and_properties_move_to_the_root_only_for_used_utilities() {
    let css = "@stillwind-slot keyframes; @variant hover { &:hover { @slot; } } \
               @utility spin { animation:turn 1s; --angle:0deg; \
               @media print { @keyframes turn { to { rotate:1turn; } } } \
               @property --angle { syntax:\"<angle>\"; inherits:false; initial-value:0deg; } } \
               @utility block { display:block; }";
    let (_, used) = compile("const x = style('spin hover:spin');", css);
    assert!(used.diagnostics.is_empty(), "{:?}", used.diagnostics);
    assert_eq!(
        used.css.matches("@keyframes turn").count(),
        1,
        "{}",
        used.css
    );
    assert_eq!(
        used.css.matches("@property --angle").count(),
        1,
        "{}",
        used.css
    );
    assert!(
        used.css.contains("@media print{@keyframes turn"),
        "{}",
        used.css
    );
    let rules = &used.css[used.css.find(".sw_").unwrap()..];
    assert!(!rules.contains("@media"), "{}", used.css);
    assert!(!rules.contains("@property"), "{}", used.css);
    let (_, unused) = compile("const x = style('block');", css);
    assert!(!unused.css.contains("@keyframes"), "{}", unused.css);
    assert!(!unused.css.contains("@property"), "{}", unused.css);
}

#[test]
fn tailwind_important_settings_apply_to_class_utilities_only() {
    let utilities = "@stillwind-slot keyframes; @utility pad { padding:4px; } \
                     @utility spin { animation:turn 1s; @keyframes turn { to { rotate:1turn; } } } \
                     @utility tone { color:red; @media print { color:blue; } }";
    let (_, important) = compile(
        "const x = style('pad tone');",
        &format!("@stillwind-plugin-important; {utilities} .applied {{ @apply pad; }}"),
    );
    assert!(
        important.diagnostics.is_empty(),
        "{:?}",
        important.diagnostics
    );
    assert!(
        important.css.contains("padding:4px!important"),
        "{}",
        important.css
    );
    assert!(
        important.css.contains("color:blue!important"),
        "{}",
        important.css
    );
    assert!(
        important.css.contains(".applied { padding:4px; }"),
        "{}",
        important.css
    );

    let (_, selector) = compile(
        "const x = style('pad spin');",
        &format!("@stillwind-plugin-important #app; {utilities}"),
    );
    assert!(
        selector.diagnostics.is_empty(),
        "{:?}",
        selector.diagnostics
    );
    assert!(selector.css.contains("#app{."), "{}", selector.css);
    assert!(!selector.css.contains("!important"), "{}", selector.css);
    assert!(
        !selector.css.contains("#app{@keyframes"),
        "{}",
        selector.css
    );
    assert!(selector.css.contains("@keyframes turn"), "{}", selector.css);
}
