//! Variant lowering and the utilities layer.
//!
//! Pins Tailwind's group, peer, and container compatibility (and the lint rule that can forbid it),
//! breakpoint queries inlined without lookup tokens, fallbacks and selector overloads from
//! CSS-defined utilities, nested supports and media rules, starting-style wrappers, and that a
//! variant wrapper is equivalent to the same prefix written on the utility.

use stillwind_compiler::CompilerOptions;
use stillwind_css::fold_calc_expressions;
use stillwind_diagnostics::{
    LintDiagnostic, LintKind, LintOverrides, LintSeverity, MarkerUseErrorKind, MarkerUseOverride,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::{CssAssertions as _, css_source};

use super::*;

#[test]
fn unresolved_variant_utility_is_omitted() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const item = style({ "hover:": "unknown-widget-class" });"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(
        !transformed.contains("unknown-widget-class"),
        "{transformed}"
    );
    assert!(!transformed.contains("className"), "{transformed}");
}

#[test]
fn compiles_tailwind_group_peer_and_container_compatibility() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const group = style("group/menu flex @container/sidebar");
export const peer = style("peer/field");
export const child = style("group-hover/menu:bg-red-500 peer-checked/field:block @sm:grid");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("group/menu"), "{transformed}");
    assert!(transformed.contains("peer/field"), "{transformed}");
    assert!(
        result
            .css
            .contains_css("container-name:sidebar;container-type:inline-size;")
    );
    assert!(result.css.contains(":is(:where(.group\\/menu):hover *)"));
    assert!(
        result
            .css
            .contains(":is(:where(.peer\\/field):checked ~ *)")
    );
    assert!(result.css.contains("@container (width >= 24rem)"));
}

#[test]
fn classes_selected_by_definitions_are_markers_beside_their_utilities() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const card = style("card");
export const title = style("card-title");
export const hovered = style({ "hover:": "card-title" });"#,
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@import \"stillwind\";
         @utility card { & .card-title { margin: 0; } }
         @utility card-title { .card & { color: red; } }",
    )
    .unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .map(|finding| &finding.diagnostic)
            .collect::<Vec<_>>(),
        [&LintDiagnostic::InvalidMarkerUse {
            utility: "card-title".into(),
            class: "card-title".into(),
            reason: MarkerUseErrorKind::Conditional,
        }]
    );
    let transformed = result.apply_to_source(&component).unwrap();
    for (name, class) in [("card", "card"), ("title", "card-title")] {
        let declaration = transformed
            .lines()
            .find(|line| line.starts_with(&format!("export const {name} ")))
            .unwrap();
        assert!(
            declaration.contains(&format!(" {class}\"")),
            "{declaration}"
        );
    }
    let hovered = transformed
        .lines()
        .find(|line| line.starts_with("export const hovered "))
        .unwrap();
    assert!(!hovered.contains("card"), "{hovered}");
    assert!(result.css.contains(" .card-title{"), "{}", result.css);
    assert!(result.css.contains(".card ."), "{}", result.css);
}

#[test]
fn ignored_marker_classes_are_not_reported() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const a = style("hover:group hover:card-title !peer");"#,
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@import \"stillwind\";
         @utility card-title { .card & { color: red; } }",
    )
    .unwrap();
    let options = CompilerOptions {
        lint: LintOverrides {
            invalid_marker_use: Some(MarkerUseOverride {
                ignored_classes: Some(vec!["group".into(), "card-title".into()]),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &options,
    );

    let classes: Vec<_> = result
        .lint_findings
        .iter()
        .map(|finding| match &finding.diagnostic {
            LintDiagnostic::InvalidMarkerUse { class, .. } => class.as_str(),
            other => panic!("unexpected finding {other:?}"),
        })
        .collect();
    assert_eq!(classes, ["peer"]);
}

#[test]
fn reports_marker_classes_that_a_candidate_cannot_add() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const a = style("hover:group !peer group/[x] -peer");
export const b = style({ "focus:": "group" });"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    let reasons: Vec<_> = result
        .lint_findings
        .iter()
        .map(|finding| match &finding.diagnostic {
            LintDiagnostic::InvalidMarkerUse { class, reason, .. } => (class.to_string(), *reason),
            other => panic!("unexpected finding {other:?}"),
        })
        .collect();
    assert_eq!(
        reasons,
        [
            ("group".into(), MarkerUseErrorKind::Conditional),
            ("peer".into(), MarkerUseErrorKind::Important),
            ("group".into(), MarkerUseErrorKind::Modifier),
            ("peer".into(), MarkerUseErrorKind::Negative),
            ("group".into(), MarkerUseErrorKind::Conditional),
        ]
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("group"), "{transformed}");
    assert!(!transformed.contains("peer"), "{transformed}");
}

#[test]
fn can_forbid_tailwind_relationship_compatibility_with_a_lint_rule() {
    let component = SourceFile::new(
        FileId(1), None, None,
        "import { style } from \"stillwind/react\"; const child = style(\"group-hover:flex @sm:grid\");",
    ).unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\"; ").unwrap();
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_typed_relationships: Some(LintSeverity::Warn),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &options,
    );

    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|finding| matches!(
                finding.diagnostic,
                LintDiagnostic::TailwindRelationshipUtility { .. }
            ))
            .count(),
        2
    );
    assert_ne!(result.css, "");
}

#[test]
fn reports_variant_selectors_that_do_not_depend_on_the_styled_element() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const host = style("[:host]:block");
export const child = style("[&:where(.card)>*]:block", { "[:host-context(.dark)]:": "flex" });
export const conditions = style("motion-reduce:block", "[@media_(hover:hover)]:flex");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let selectors = result
        .lint_findings
        .iter()
        .filter_map(|finding| match &finding.diagnostic {
            LintDiagnostic::DocumentScopedSelector { selector, .. } => Some(selector.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selectors, vec![":host", ":host-context(.dark)"]);
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::DocumentScopedSelector),
        LintSeverity::Deny
    );
    assert!(
        result.css.contains(":host{display:block;}"),
        "{}",
        result.css
    );
}

#[test]
fn reports_variant_selectors_that_repeat_the_styled_element() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
export const adjacent = style("[&+&]:block", "group-[&+&]:flex", "not-[&+&]:grid", "pair:hidden");
export const nested = style({ "[&_&]:": "flex" });
export const single = style("[&:where(.a,.b)]:block", "group-hover:flex", "not-[.a_&]:grid");"#,
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@import \"stillwind\"; @custom-variant pair (& ~ &);",
    )
    .unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let mut selectors = result
        .lint_findings
        .iter()
        .filter_map(|finding| match &finding.diagnostic {
            LintDiagnostic::RepeatedNestingSelector { selector } => Some(selector.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    selectors.sort_unstable();
    assert_eq!(selectors, vec!["& &", "& ~ &", "&+&", "&+&", "&+&"]);
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::RepeatedNestingSelector),
        LintSeverity::Warn
    );
    assert!(result.css.contains(":not(*+*)"), "{}", result.css);
}

#[test]
fn host_settings_can_disable_the_element_scope_rule() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; export const host = style(\"[:host]:block\");",
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\"; ").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions {
            lint: LintOverrides {
                document_scoped_selector: Some(LintSeverity::Off),
                ..Default::default()
            },
            ..Default::default()
        },
    );

    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::DocumentScopedSelector),
        LintSeverity::Off
    );
}

/// A transition class participates only through a name, so a call states exactly one source
/// of it; a name the compiler cannot see stays invisible, which is why both rules warn.
#[test]
fn reports_missing_and_duplicated_view_transition_names() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const title = style.viewTransition({ old: "opacity-0" });
export const unnamed = style("block", title);
export const named = style("block", title.for("hero"));
export const optedOut = style("block", title, "[view-transition-name:none]");
export const doubled = style("block", title.for("hero"), "[view-transition-name:other]");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let missing = result
        .lint_findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.diagnostic,
                LintDiagnostic::MissingViewTransitionName
            )
        })
        .count();
    let duplicated = result
        .lint_findings
        .iter()
        .filter_map(|finding| match finding.diagnostic {
            LintDiagnostic::DuplicateViewTransitionName { count } => Some(count),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(missing, 1, "{:#?}", result.lint_findings);
    assert_eq!(duplicated, vec![2], "{:#?}", result.lint_findings);
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::MissingViewTransitionName),
        LintSeverity::Warn
    );
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::DuplicateViewTransitionName),
        LintSeverity::Warn
    );
}

#[test]
fn theme_gate_emits_only_required_default_tokens_and_keyframes() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; const props = style(\"p-4 animate-spin\");",
    )
    .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &ModuleId::from("test.tsx"), ScriptLanguage::Tsx, ScriptModuleKind::Unambiguous, &css_source(
            FileId(2),
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap(), &CssSourceName::from("stillwind.css"), &CompilerOptions::default());

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains_css("--spacing:0.25rem;"));
    assert!(
        result
            .css
            .contains_css("--animate-spin:spin 1s linear infinite;")
    );
    assert!(result.css.contains("@keyframes spin {"));
    assert!(!result.css.contains("--color-red-500:"));
    assert!(!result.css.contains("@keyframes pulse {"));
}

#[test]
fn breakpoint_variants_inline_queries_without_emitting_lookup_tokens() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; const props = style(\"foo:grid 東京:flex\");",
    )
    .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &ModuleId::from("test.tsx"), ScriptLanguage::Tsx, ScriptModuleKind::Unambiguous, &css_source(
            FileId(2),
            "@import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\"; @theme { --breakpoint-foo: 60rem; --breakpoint-東京: 70rem; }",
        ).unwrap(), &CssSourceName::from("stillwind.css"), &CompilerOptions::default());

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("@media (width >= 60rem)"),
        "{}",
        result.css
    );
    assert!(result.css.contains("@media (width >= 70rem)"));
    assert!(!result.css.contains("--breakpoint-foo:"));
    assert!(!result.css.contains("--breakpoint-東京:"));
}

#[test]
fn utility_ir_gate_keeps_fallbacks_and_selector_overloads() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; const props = style(\"paint-card\");",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &css_source(
            FileId(2),
            r#"@import "stillwind/utilities.css" layer(utilities);
@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
        )
        .unwrap(),
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let class_name = &result.classes[0].class_name;
    assert!(
        result.css.contains(&format!(
            ".{class_name}{{color: red; color: color(display-p3 1 0 0);}}"
        )),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(".{class_name}::before{{content: \"fallback\";}}"))
    );
}

#[test]
fn emits_nested_supports_from_css_utility_definitions() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("bg-linear-to-r");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains_css("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains("@supports (background: linear-gradient")
    );
    assert!(
        result
            .css
            .contains_css("--tw-gradient-position:to right in oklab;")
    );
    assert!(
        result
            .css
            .contains_css("--tw-gradient-position:to right in oklab;")
    );
}

#[test]
fn emits_nested_media_from_css_utility_definitions() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("outline-hidden");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains(
            "@media (forced-colors: active){outline-offset:2px;outline:2px solid transparent;}"
        ),
        "{}",
        result.css
    );
    assert!(result.css.contains_css("outline:2px solid transparent;"));
}

#[test]
fn emits_selector_bearing_css_utilities_on_their_target_children() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("divide-x-[2px]");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains(":where(.")
            && result.css.contains(
                "border-inline-end-style:var(--tw-border-style);border-inline-end-width:2px;"
            ),
        "{}",
        result.css
    );
}

#[test]
fn emits_starting_style_variants_as_a_css_wrapper() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("starting:opacity-0");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@starting-style{.sw_"));
    assert!(
        fold_calc_expressions(&result.css, None).contains("{opacity:0%;}"),
        "{}",
        result.css
    );
}

#[test]
fn variant_wrappers_are_equivalent_to_prefixed_utility_lists() {
    let theme = css_source(
        FileId(2),
        r#"@import "stillwind";
@utility text-red { color: red; }
@utility text-blue { color: blue; }
"#,
    )
    .unwrap();
    for (wrapped, flat) in [
        (
            r#"{ "hover:": ["text-red", { "[&:where(.mod)]:": "text-blue" }] }"#,
            r#""hover:text-red hover:[&:where(.mod)]:text-blue""#,
        ),
        (
            r#"{ "md:hover:": ["text-red text-blue", false, null, undefined] }"#,
            r#""md:hover:text-red md:hover:text-blue""#,
        ),
        (
            r#"{ "aria-busy:": "text-red", "data-[state=open]:": "text-blue" }"#,
            r#""aria-busy:text-red data-[state=open]:text-blue""#,
        ),
        (
            r#"{ "hover:": [active && "text-red", active ? "text-blue" : "text-red"] }"#,
            r#"active && "hover:text-red", active ? "hover:text-blue" : "hover:text-red""#,
        ),
        (
            r#"{ "[&>p]:": "divide-x-2 hover:text-red" }"#,
            r#""[&>p]:divide-x-2 [&>p]:hover:text-red""#,
        ),
    ] {
        let compile = |args: &str| {
            let source = SourceFile::new(FileId(1), None, None, format!(
                "import {{ style }} from \"stillwind/react\"; declare const active: boolean; const props = style({args});"
            )).unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &theme,
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions::default(),
            );
            assert!(
                result.diagnostics.is_empty(),
                "{args}: {:#?}",
                result.diagnostics
            );
            (result.css.clone(), result.apply_to_source(&source).unwrap())
        };
        let (css, code) = compile(wrapped);
        let (flat_css, flat_code) = compile(flat);
        assert_eq!(css, flat_css, "{wrapped}");
        // Wrapper spellings keep their array shape, so compare the generated
        // class names rather than the emitted expression text.
        let classes = |code: &str| {
            let mut names = code
                .split('"')
                .filter(|part| part.starts_with("sw_"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            names.sort();
            names
        };
        assert_eq!(classes(&code), classes(&flat_code), "{wrapped}");
    }
}
