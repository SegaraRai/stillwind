//! Animation and view-transition compilation.
//!
//! Pins keyframe emission for local `@utility` frames, selector-scoped keyframes uses, keyed view
//! transitions, the independent `--sw-` and `--tw-` prefix configuration, and the rejection of a
//! second view transition in one call.

use stillwind_compiler::CompilerOptions;
use stillwind_css::fold_calc_expressions;
use stillwind_diagnostics::{CompileErrorKind, LintKind};
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn compiles_local_utility_animations_and_selector_uses() {
    let component = SourceFile::new(
        FileId(1),
        "src/Dialog.tsx",
        r#"import { style } from "stillwind/react";
const enter = style.keyframes({ from: "opacity-0 translate-y-2 scale-98", to: "opacity-100 translate-y-0 scale-100" });
const exit = style.keyframes({ from: "opacity-100", to: "opacity-0 scale-98" });
const panel = style("rounded-md", enter("duration-180 ease-standard both"), { "[&[data-state=closed]]:": exit("duration-120 ease-standard both") });"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --radius-md: .375rem; --spacing-2: .5rem; --ease-standard: ease-out; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@keyframes sw-kf-"));
    assert!(
        fold_calc_expressions(&result.css)
            .contains("from{--tw-translate-y:var(--spacing-2);translate:var(--tw-translate-x) var(--tw-translate-y);--tw-scale-x:98%;--tw-scale-y:98%;--tw-scale-z:98%;scale:var(--tw-scale-x) var(--tw-scale-y);opacity:0%;}"),
        "{}",
        result.css
    );
    assert!(result.css.contains("animation-name:sw-kf-"));
    assert!(result.css.contains("animation-duration:"));
    assert!(
        result
            .css
            .contains("[data-state=closed]{animation-name:sw-kf-")
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.keyframes"));
    assert!(transformed.contains("const enter = undefined;"));
}

/// A declaration's label is spelled into every name the declaration generates, in the family's
/// own separator, and the rendering positions write it the way their grammar requires.
#[test]
fn spells_declaration_labels_into_the_generated_names() {
    let component = SourceFile::new(
        FileId(1),
        "src/Labelled.tsx",
        r#"import { style } from "stillwind/react";
const enter = style.keyframes({ label: "fade", from: "opacity-0", to: "opacity-100" });
const panel = style.viewTransition({ label: "panel", old: "opacity-0" });
export const item = style(panel, enter("duration-180"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("@keyframes sw-kf-fade-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("animation-name:sw-kf-fade-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("::view-transition-old(.sw-vt-panel-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("view-transition-class:sw-vt-panel-"),
        "{}",
        result.css
    );
    // The declaration value carries the same token the rules select.
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("{v:\"sw-vt-panel-"), "{transformed}");
    assert!(!transformed.contains("style.keyframes"), "{transformed}");
}

#[test]
fn compiles_view_transition_assets_and_keyed_instances() {
    let component = SourceFile::new(
        FileId(1),
        "src/ProductCard.tsx",
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const title = style.viewTransition({ group: "duration-180 ease-standard", old: "opacity-0 -translate-y-1", new: "opacity-100" });
const card = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
const heading = style("text-accent", title);
const item = style("rounded-md", card.for(product.id));
const image = style(title);
const detail = style(title);"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --radius-md: .375rem; --spacing-1: .25rem; --ease-standard: ease-out; }",
    ).unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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
    // A declaration is a plain value, and only a runtime key writes a name inline.
    assert!(
        transformed.contains("const title = {v:\"sw-vt-"),
        "{transformed}"
    );
    assert!(
        transformed.contains("{[stillwindStyleKey]:{\"view-transition-name\":"),
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
        FileId(1),
        "src/Gallery.tsx",
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ old: "opacity-0 motion-reduce:opacity-100", new: "opacity-100", imagePair: "isolation-auto" });
export const photo = style(item);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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
        FileId(1),
        "src/Reduced.tsx",
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ group: "duration-180 ease-linear both motion-reduce:duration-0", old: "opacity-0", new: "opacity-100" });
export const photo = style(item);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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

/// A projected property rides the same path: the condition is stripped before resolution and the
/// projected declaration lands in the conditioned rule, while a property no mapping names still
/// fails the option.
#[test]
fn conditions_projected_group_options() {
    let component = SourceFile::new(
        FileId(1),
        "src/Projected.tsx",
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ group: "motion-reduce:[transition-duration:75ms]", old: "opacity-0" });
export const photo = style(item);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("@media (prefers-reduced-motion: reduce){::view-transition-group(.sw-vt-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("{ animation-duration:75ms;}"),
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
        "src/Composed.tsx",
        r#"import { style } from "stillwind/react";
const base = style.viewTransition({ old: "opacity-0" });
const other = style.viewTransition({ new: "opacity-100" });
const block = style("block", base);
export const replaced = style(block, other.for("hero"));
export const kept = style(block, base.for("hero"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let findings = result
        .lint_findings
        .iter()
        .filter(|finding| finding.kind == LintKind::SplitViewTransitionClasses)
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{:#?}", result.lint_findings);
    assert_eq!(result.lint_policy["single-view-transition-class"], "warn");
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
        "src/Runtime.tsx",
        r#"import { style } from "stillwind/react";
declare const key: string;
const transition = style.viewTransition({ old: "opacity-0" });
const named = style(transition.for(key));
export const props = style(named, "flex");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(
        result
            .diagnostics
            .iter()
            .any(|error| matches!(error.kind, CompileErrorKind::StyleBaseWithProps)),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn configures_stillwind_and_tailwind_compatibility_prefixes_independently() {
    let component = SourceFile::new(
        FileId(1),
        "src/Prefix.tsx",
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
export const item = style("translate-x-1", card);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-1: .25rem; }",
    )
    .unwrap();
    let options = CompilerOptions {
        prefix: "--app-".to_owned(),
        tailwind_prefix: "--compat-".to_owned(),
        ..CompilerOptions::default()
    };

    let result =
        Compiler::compile_source_with_options(&OxcSourceAnalyzer, &component, &theme, &options);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    // The class token is not a custom property, so the custom-property prefix does not
    // namespace it; the compatibility prefix still reaches the Tailwind properties.
    assert!(result.css.contains("::view-transition-old(.sw-vt-"));
    assert!(result.css.contains("view-transition-class:sw-vt-"));
    assert!(result.css.contains("--compat-translate-x"));
    assert!(!result.css.contains("--sw-"));
    assert!(!result.css.contains("--tw-"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("--sw-"));
}

/// A guarded use fills a slot its own class carries, and the unit owning the element class
/// list references every slot with an empty fallback, so a condition contributes its token
/// without replacing what another condition contributed.
#[test]
fn compiles_guarded_view_transitions_through_class_slots() {
    let component = SourceFile::new(
        FileId(1),
        "src/Guarded.tsx",
        r#"import { style } from "stillwind/react";
declare const compact: boolean;
const main = style.viewTransition({ old: "opacity-0" });
const sub = style.viewTransition({ new: "opacity-100" });
export const item = style("rounded", compact && sub, main.for("hero"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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

/// Two conditions that hold together each fill their own slot, so the resolved class list
/// is the union of what the element declares rather than the last one to apply.
#[test]
fn unions_guarded_view_transitions_through_class_slots() {
    let component = SourceFile::new(
        FileId(1),
        "src/Union.tsx",
        r#"import { style } from "stillwind/react";
declare const first: boolean;
declare const second: boolean;
const one = style.viewTransition({ old: "opacity-0" });
const two = style.viewTransition({ new: "opacity-100" });
export const item = style("block", first && one, second && two);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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

/// A `.for(...)` outside a call builds a value: a literal key folds into a record, and a
/// key only the runtime knows stays a call the runtime resolves.
#[test]
fn lowers_view_transition_key_applications_outside_calls() {
    let component = SourceFile::new(
        FileId(1),
        "src/Keyed.tsx",
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const card = style.viewTransition({ old: "opacity-0" });
export const literal = card.for("hero");
export const dynamic = card.for(product.id);
export const useLiteral = style("block", literal);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(
        transformed.contains("const literal = {v:\"sw-vt-"),
        "{transformed}"
    );
    assert!(transformed.contains(",w:\"hero\"}"), "{transformed}");
    assert!(
        transformed.contains("stillwindViewTransitionKey(card,product.id)"),
        "{transformed}"
    );
    assert!(
        transformed.contains("stillwindViewTransitionName(literal)"),
        "{transformed}"
    );
}

#[test]
fn unions_view_transition_classes_in_one_call() {
    let component = SourceFile::new(
        FileId(1),
        "src/MultipleTransitions.tsx",
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
const image = style.viewTransition({ new: "opacity-100" });
export const item = style(card, image);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

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
