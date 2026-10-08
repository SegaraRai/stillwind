//! Checks guarded transitions and runtime participant names.

use super::*;

/// A guarded use fills a slot its own class carries, and the unit owning the element class
/// list references every slot with an empty fallback, so a condition contributes its token
/// without replacing what another condition contributed.
#[test]
fn compiles_guarded_view_transitions_through_class_slots() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const compact: boolean;
const main = style.viewTransition({ old: "opacity-0" });
const sub = style.viewTransition({ new: "opacity-100" });
export const item = style("rounded", compact && sub, main.for("hero"));"#,
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
    assert_eq!(
        result.css.matches("var(--sw-vtc-").count(),
        1,
        "{}",
        result.css
    );
    // One slot reference in the class list and one assignment in the guarded unit.
    assert_eq!(result.css.matches("--sw-vtc-").count(), 2, "{}", result.css);
    assert!(
        result.css.contains("view-transition-name:sw-vt-"),
        "{}",
        result.css
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("(compact) &&"), "{transformed}");
}

#[test]
fn variant_transition_dynamic_name_stays_in_the_source_condition() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const enabled: boolean;
declare const id: string;
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style("block", { "hover:": enabled && motion.for(id) });"#,
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
    assert!(result.css.contains(":hover"), "{}", result.css);
    assert!(
        result.css.contains("view-transition-name:var(--sw-vtc-"),
        "{}",
        result.css
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("(enabled) &&"), "{transformed}");
    assert!(
        transformed.contains("stillwindViewTransitionName("),
        "{transformed}"
    );
    assert!(
        !transformed.contains("\"view-transition-name\":stillwindViewTransitionName"),
        "{transformed}"
    );
}

#[test]
fn conditional_transition_dynamic_name_uses_inline_style() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const enabled: boolean;
declare const id: string;
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style("block", enabled && motion.for(id));"#,
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
    assert!(transformed.contains("(enabled) &&"), "{transformed}");
    assert!(
        transformed.contains("[stillwindStyleKey]:{\"view-transition-name\":"),
        "{transformed}"
    );
    assert!(
        !result.css.contains("view-transition-name:var("),
        "{}",
        result.css
    );
}

#[test]
fn variant_transition_literal_name_needs_no_inline_style() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const enabled: boolean;
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style("block", { "hover:": enabled && motion.for("hero") });"#,
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
    assert!(result.css.contains(":hover"), "{}", result.css);
    assert!(
        result.css.contains("view-transition-name:sw-vt-"),
        "{}",
        result.css
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style:"), "{transformed}");
    assert!(transformed.contains("stillwindClsx("), "{transformed}");
}

#[test]
fn dynamic_names_in_distinct_variants_have_distinct_slots() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const first: string;
declare const second: string;
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style({ "hover:": motion.for(first), "focus:": motion.for(second) });"#,
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
    let names = result
        .css
        .split("view-transition-name:var(")
        .skip(1)
        .map(|rule| rule.split(')').next().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 2, "{}", result.css);
    assert_ne!(names[0], names[1], "{}", result.css);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains(&names[0]), "{transformed}");
    assert!(transformed.contains(&names[1]), "{transformed}");
}

#[test]
fn variant_transition_name_supports_string_style_adapters() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/svelte";
declare const id: string;
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style({ "hover:": motion.for(id) });"#,
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
        result.css.contains("view-transition-name:var(--sw-vtc-"),
        "{}",
        result.css
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(
        transformed.contains("stillwindViewTransitionName("),
        "{transformed}"
    );
    assert!(transformed.contains("--sw-vtc-"), "{transformed}");
}

#[test]
fn mixed_array_keeps_props_inputs_and_conditional_transition() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
declare const enabled: boolean;
declare const id: string;
const anchor = style.ref();
const motion = style.viewTransition({ old: "opacity-0" });
export const item = style([anchor, { "aria-busy": enabled, "--size": 4, class: "external" }, enabled && motion.for(id)]);"#,
    ).unwrap();
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
    assert!(transformed.contains("aria-busy"), "{transformed}");
    assert!(transformed.contains("--size"), "{transformed}");
    assert!(transformed.contains("external"), "{transformed}");
    assert!(transformed.contains("(enabled) &&"), "{transformed}");
    assert!(
        transformed.contains("stillwindViewTransitionName("),
        "{transformed}"
    );
}

/// Two conditions that hold together each fill their own slot, so the resolved class list
/// is the union of what the element declares rather than the last one to apply.
#[test]
fn unions_guarded_view_transitions_through_class_slots() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const first: boolean;
declare const second: boolean;
const one = style.viewTransition({ old: "opacity-0" });
const two = style.viewTransition({ new: "opacity-100" });
export const item = style("block", first && one, second && two);"#,
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
    assert_eq!(
        result.css.matches("var(--sw-vtc-").count(),
        2,
        "{}",
        result.css
    );
    assert_eq!(result.css.matches("--sw-vtc-").count(), 4, "{}", result.css);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("(first) &&"), "{transformed}");
    assert!(transformed.contains("(second) &&"), "{transformed}");
}
