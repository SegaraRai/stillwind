//! Checks transition phases, conditions, asset uses, and composition.

use stillwind_diagnostics::{LintDiagnostic, LintKind};

use super::*;

#[test]
fn compiles_view_transition_assets_and_keyed_instances() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
export const title = style.viewTransition({ group: "duration-180 ease-standard", old: "opacity-0 -translate-y-1", new: "opacity-100" });
const card = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
const heading = style("text-accent", title);
const item = style("rounded-md", card.for(product.id));
const image = style(title);
const detail = style(title);"#,
    ).unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --color-accent: red; --radius-md: .375rem; --spacing-1: .25rem; --ease-standard: ease-out; }",
    ).unwrap();

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
    assert!(result.css.contains("::view-transition-group(.sw-vt-"));
    assert!(result.css.contains("::view-transition-old(.sw-vt-"));
    assert!(result.css.contains("::view-transition-new(.sw-vt-"));
    // The token is the class itself; no custom-property indirection remains.
    assert!(result.css.contains("view-transition-class:sw-vt-"));
    assert!(!result.css.contains("--sw-vt-name"));
    assert!(!result.css.contains("--sw-vt-class"));
    let repeated_class = result.classes.last().expect("a class").class_name.as_str();
    assert_eq!(
        result
            .module_css
            .matches(&format!(".{repeated_class}"))
            .count(),
        1,
        "an identical view-transition class should only be emitted once"
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.viewTransition"));
    // An exported declaration is a plain value, and only a runtime key writes a name inline.
    assert!(
        transformed.contains("export const title = {v:\"sw-vt-"),
        "{transformed}"
    );
    assert!(
        transformed.contains("style:{\"view-transition-name\":"),
        "{transformed}"
    );
    assert_eq!(
        transformed.matches("stillwindViewTransitionName(").count(),
        1
    );
    assert!(!transformed.contains("stillwindViewTransitionStyle"));
    assert_eq!(transformed.matches("product.id").count(), 1);
}

#[test]
fn compiles_view_transition_conditions_and_image_pair_phases() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ old: "opacity-0 motion-reduce:opacity-100", new: "opacity-100", imagePair: "isolation-auto" });
export const photo = style(item);"#,
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
        result.css.contains("::view-transition-image-pair(.sw-vt-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains_css("isolation:isolate;") || result.css.contains_css("isolation:auto;"),
        "{}",
        result.css
    );
    let base = result
        .css
        .find("::view-transition-old(.sw-vt-")
        .expect("base old rule");
    let conditional = result
        .css
        .find("@media (prefers-reduced-motion: reduce){::view-transition-old(.sw-vt-")
        .expect("conditional old rule");
    assert!(base < conditional, "{}", result.css);
}

/// A group option may carry a media or supports condition: the unconditional options share one
/// rule and each conditioned option follows as its own rule with the same selector, so the
/// condition changes only the properties that option states.
#[test]
fn conditions_group_options_on_their_own_rules() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ group: "duration-180 ease-linear both motion-reduce:duration-0", old: "opacity-0", new: "opacity-100" });
export const photo = style(item);"#,
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
        result.css.contains("::view-transition-group(.sw-vt-"),
        "{}",
        result.css
    );
    assert_eq!(
        result.css.matches("::view-transition-group(").count(),
        2,
        "{}",
        result.css
    );
    assert!(
        result.css.contains(
            "{ animation-duration:calc(180 * 1ms);animation-timing-function:linear;animation-fill-mode:both;}"
        ),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains("@media (prefers-reduced-motion: reduce){::view-transition-group(.sw-vt-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("{ animation-duration:calc(0 * 1ms);}"),
        "{}",
        result.css
    );
}

/// Timing utilities of the `view-transition` context set the animation of the
/// pseudo-element, and a condition wraps the rule that carries the conditioned
/// declaration.
#[test]
fn conditions_group_timing_utilities() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ group: "motion-reduce:duration-75", old: "opacity-0" });
export const photo = style(item);"#,
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
        result
            .css
            .contains("@media (prefers-reduced-motion: reduce){::view-transition-group(.sw-vt-"),
        "{}",
        result.css
    );
    assert!(
        fold_calc_expressions(&result.css, None).contains("{ animation-duration:75ms;}"),
        "{}",
        result.css
    );
}

/// A composed base's transition classes do not merge with the derived call's own list: the
/// derived rules come later and replace the single class property, so a transition only the base
/// declares stops marking the element. Re-declaring the base's own transition loses nothing.
#[test]
fn warns_when_a_composed_base_declares_other_transition_classes() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const base = style.viewTransition({ old: "opacity-0" });
const other = style.viewTransition({ new: "opacity-100" });
const block = style("block", base);
export const replaced = style(block, other.for("hero"));
export const kept = style(block, base.for("hero"));"#,
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
    let findings = result
        .lint_findings
        .iter()
        .filter(|finding| finding.diagnostic == LintDiagnostic::SplitViewTransitionClass)
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{:#?}", result.lint_findings);
    assert_eq!(
        result
            .lint_settings
            .reporting_severity(LintKind::SplitViewTransitionClass),
        LintSeverity::Warn
    );
    assert_eq!(
        component.slice(findings[0].labels[0].span).unwrap(),
        "style(block, other.for(\"hero\"))"
    );
}

/// A transition name the runtime composes reaches the element as an inline property, so a base
/// carrying one still cannot be composed: merging two payloads is what the rejection covers.
#[test]
fn rejects_composing_a_base_with_a_runtime_transition_name() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const key: string;
const transition = style.viewTransition({ old: "opacity-0" });
const named = style(transition.for(key));
export const props = style(named, "flex");"#,
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

    assert!(
        result
            .lint_findings
            .iter()
            .any(|error| matches!(error.diagnostic, LintDiagnostic::StyleBaseWithInlineStyle)),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_view_transition_key_applications_outside_calls() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const card = style.viewTransition({ old: "opacity-0" });
export const literal = card.for("hero");
export const dynamic = card.for(product.id);
export const useLiteral = style("block", literal);"#,
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

    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|error| matches!(
                error.diagnostic,
                CompileDiagnostic::UnconsumedMacroUse { .. }
            ))
            .count(),
        2,
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.transforms, [] as [SourceTransform; 0]);
}

#[test]
fn unions_view_transition_classes_in_one_call() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
const image = style.viewTransition({ new: "opacity-100" });
export const item = style(card, image);"#,
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
    assert_eq!(
        result.css.matches("view-transition-class:").count(),
        1,
        "{}",
        result.css
    );
    let declaration = result
        .css
        .split("view-transition-class:")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("a class list");
    let tokens = declaration
        .split_whitespace()
        .filter(|token| token.starts_with("sw-vt-"))
        .count();
    assert_eq!(tokens, 2, "{}", result.css);
}
