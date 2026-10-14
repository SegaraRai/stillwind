//! Checks ref conditions, scope combinations, and selector targets.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

fn compile_registry(code: &str, css: &str) -> (SourceFile, CompileResult) {
    let css = css_source(
        FileId(2),
        format!("@import \"stillwind/builtins.css\"; {css}"),
    )
    .unwrap();
    compile(code, css.text())
}

#[test]
fn scoped_presence_call_tag_and_shorthand_emit_equivalent_conditions() {
    let mut selectors = Vec::new();
    for expression in ["item().in(parent)", "item``.in(parent)", "item.in(parent)"] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{expression}: {result:#?}");
        let parent = marker(&result, "parent");
        let item = marker(&result, "item");
        assert!(
            result.css.contains(&format!(":has(:is(:where(.{item})")),
            "{}",
            result.css
        );
        assert!(
            result.css.contains(&format!(":where(.{parent})")),
            "{}",
            result.css
        );
        selectors.push(result.css);
    }
    assert_eq!(selectors[0], selectors[1]);
    assert_eq!(selectors[0], selectors[2]);
}

#[test]
fn stateful_scope_prefixes_apply_to_the_scope_and_ref_elements() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('hover').in(parent('focus-within'))]: 'active:a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let parent = marker(&result, "parent");
    let item = marker(&result, "item");
    assert!(
        result
            .css
            .contains(&format!(":where(.{parent}):focus-within")),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(":has(:is(:where(.{item}):hover")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("@media (hover: hover)"),
        "{}",
        result.css
    );
    assert!(result.css.contains(":active:is("), "{}", result.css);
}

#[test]
fn custom_ref_and_scope_alternatives_expand_each_combination() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('hocus').in(parent('hocus'))]: 'a'});",
        "@variant hocus { &:hover { @slot; } &:focus { @slot; } } @utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    for item_state in ["hover", "focus"] {
        for parent_state in ["hover", "focus"] {
            assert!(
                result.css.split('{').any(|selector| selector
                    .contains(&format!(":has(:is(:where(.{item}):{item_state}))"))
                    && selector.contains(&format!(":is(:where(.{parent}):{parent_state})"))),
                "{}",
                result.css
            );
        }
    }
}

#[test]
fn repeated_scopes_conjoin_independently_of_call_order() {
    let mut css = Vec::new();
    for chain in [
        "item().in(inner('focus')).in(outer('active'))",
        "item().in(outer('active')).in(inner('focus'))",
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const outer=style.ref('outer'); const inner=style.ref('inner'); const item=style.ref('item'); const x=style({{[{chain}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{chain}: {result:#?}");
        let outer = marker(&result, "outer");
        let inner = marker(&result, "inner");
        let item = marker(&result, "item");
        let selector = result
            .css
            .split('{')
            .find(|part| part.contains(&item))
            .expect("utility selector");
        assert!(
            selector.contains(&format!(":where(.{outer}):active")),
            "{selector}"
        );
        assert!(
            selector.contains(&format!(":where(.{inner}):focus")),
            "{selector}"
        );
        assert!(selector.contains(":has("), "{selector}");
        assert!(!selector.contains(','), "{selector}");
        assert_eq!(
            selector
                .matches(&format!(":has(:is(:where(.{item})"))
                .count(),
            2,
            "{selector}"
        );
        css.push(result.css);
    }
    assert_eq!(css[0], css[1]);
}

#[test]
fn equivalent_finite_scope_conditions_expand_once_when_repeated() {
    for stateful in [false, true] {
        let mut outputs = Vec::new();
        for (count, alternate_expression) in [(1, false), (12, false), (12, true)] {
            let chain = (0..count)
                .map(|index| {
                    let choice = if alternate_expression && index % 2 == 1 {
                        "otherFlag ? t : s"
                    } else {
                        "flag ? s : t"
                    };
                    if stateful {
                        format!(".in(({choice})('focus:hover'))")
                    } else {
                        format!(".in({choice})")
                    }
                })
                .collect::<String>();
            let (_, result) = compile_registry(
                &format!(
                    "const r=style.ref('r'); const s=style.ref('s'); const t=style.ref('t'); const x=style({{[r(){chain}]: 'a'}});"
                ),
                "@utility a { color:red; }",
            );
            assert!(
                result.diagnostics.is_empty(),
                "{count}, {stateful}: {:?}",
                result.diagnostics
            );
            assert_eq!(
                result.css.matches("color:red;").count(),
                2,
                "{count}, {stateful}"
            );
            let r = marker(&result, "r");
            let s = marker(&result, "s");
            let t = marker(&result, "t");
            let selectors = result
                .css
                .split('{')
                .filter(|part| part.contains(&r))
                .collect::<Vec<_>>();
            assert_eq!(selectors.len(), 2);
            for selector in selectors {
                assert_eq!(selector.matches(":has(").count(), 1, "{selector}");
                assert_ne!(selector.contains(&s), selector.contains(&t), "{selector}");
                if stateful {
                    assert!(selector.contains(":focus:hover"), "{selector}");
                }
            }
            let mut css = result.css;
            for (index, class) in result.classes.iter().enumerate() {
                css = css.replace(&class.class_name, &format!("test_class_{index}"));
            }
            outputs.push(css);
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_eq!(outputs[0], outputs[2]);
    }
}

#[test]
fn distinct_finite_scope_conditions_expand_their_cartesian_product() {
    let (_, result) = compile(
        "const r=style.ref('r'); const s=style.ref('s'); const t=style.ref('t'); const u=style.ref('u'); const v=style.ref('v'); const x=style({[r().in(flag ? s : t).in(otherFlag ? u : v)]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("color:red;").count(), 4);
    let reference = marker(&result, "r");
    let first_scopes = [marker(&result, "s"), marker(&result, "t")];
    let second_scopes = [marker(&result, "u"), marker(&result, "v")];
    let selectors = result
        .css
        .split('{')
        .filter(|part| part.contains(&reference))
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), 4);
    for first in &first_scopes {
        for second in &second_scopes {
            let selector = selectors
                .iter()
                .find(|selector| selector.contains(first) && selector.contains(second))
                .expect("scope combination");
            assert_eq!(selector.matches(":has(").count(), 2, "{selector}");
            assert_ne!(
                selector.contains(&first_scopes[0]),
                selector.contains(&first_scopes[1]),
                "{selector}"
            );
            assert_ne!(
                selector.contains(&second_scopes[0]),
                selector.contains(&second_scopes[1]),
                "{selector}"
            );
        }
    }
}

#[test]
fn a_container_query_inside_a_scoped_ref_variant_stays_relative_to_the_target() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const card=style.ref('card'); const root=style(card,card.asContainer('size')); const parentStyle=style(parent); const itemStyle=style(item); const x=style({[item('hover').in(parent('focus'))]: {[card.at('width > 20rem')]: 'a'}});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.lint_findings.is_empty(), "{result:#?}");
    let name = result
        .css
        .split("container-name:")
        .nth(1)
        .and_then(|part| part.split(';').next())
        .expect("container name");
    assert!(
        result
            .css
            .contains(&format!("@container {name} (width > 20rem){{")),
        "{}",
        result.css
    );
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    assert!(
        result
            .css
            .contains(&format!(":has(:is(:where(.{item}):hover")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(&format!(":where(.{parent}):focus")),
        "{}",
        result.css
    );
}

#[test]
fn scoped_refs_and_container_queries_test_the_final_child_target() {
    let mut outputs = Vec::new();
    for input in [
        "{[item('hover').in(parent)]: {[card.at('width > 20rem')]: '*:a'}}",
        "{[card.at('width > 20rem')]: {[item('hover').in(parent)]: '*:a'}}",
        "{'*:': {[item('hover').in(parent)]: {[card.at('width > 20rem')]: 'a'}}}",
        "`${item('hover').in(parent)}:${card.at('width > 20rem')}:*:a`",
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref('parent'); const item=style.ref('item'); const card=style.ref('card'); const provider=style(card,card.asContainer('size')); const x=style({input});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        let item = marker(&result, "item");
        let parent = marker(&result, "parent");
        let class = &result.classes.last().expect("target class").class_name;
        assert!(
            result.css.contains(&format!(
                ":is(.{class} > *):is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)"
            )),
            "{input}: {}",
            result.css
        );
        assert!(result.css.contains("@container sw-c-"), "{}", result.css);
        let mut css = result.css;
        for (index, class) in result.classes.iter().enumerate() {
            css = css.replace(&class.class_name, &format!("test_class_{index}"));
        }
        outputs.push(css);
    }
    for output in &outputs[1..] {
        assert_eq!(output, &outputs[0]);
    }
}

#[test]
fn ref_conditions_guard_the_element_that_owns_before_and_after() {
    for (expression, scoped) in [("item('hover')", false), ("item('hover').in(parent)", true)] {
        for pseudo in ["before", "after"] {
            let (_, result) = compile_registry(
                &format!(
                    "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: '{pseudo}:a'}});"
                ),
                "@utility a { color:red; }",
            );
            assert!(
                result.diagnostics.is_empty(),
                "{expression}, {pseudo}: {result:#?}"
            );
            let item = marker(&result, "item");
            let guard = if scoped {
                let parent = marker(&result, "parent");
                format!(":is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)")
            } else {
                format!(":is(:is(:where(.{item}):hover) *)")
            };
            let class = &result.classes[0].class_name;
            assert!(
                result.css.contains(&format!(".{class}{guard}::{pseudo}{{")),
                "{}",
                result.css
            );
            assert!(result.css.contains("color:red;"), "{}", result.css);
        }
    }
}

#[test]
fn ref_conditions_guard_the_final_child_before_its_pseudo_element() {
    for (expression, scoped) in [("item('hover')", false), ("item('hover').in(parent)", true)] {
        for pseudo in ["before", "after"] {
            let (_, result) = compile_registry(
                &format!(
                    "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: 'child-{pseudo}:a'}});"
                ),
                &format!(
                    "@variant child-{pseudo} {{ & > *::{pseudo} {{ @slot; }} }} @utility a {{ color:red; }}"
                ),
            );
            assert!(
                result.diagnostics.is_empty(),
                "{expression}, {pseudo}: {result:#?}"
            );
            let item = marker(&result, "item");
            let guard = if scoped {
                let parent = marker(&result, "parent");
                format!(":is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)")
            } else {
                format!(":is(:is(:where(.{item}):hover) *)")
            };
            let class = &result.classes[0].class_name;
            assert!(
                result
                    .css
                    .contains(&format!(".{class} > *{guard}::{pseudo}{{")),
                "{}",
                result.css
            );
            assert!(result.css.contains("color:red;"), "{}", result.css);
        }
    }
}

#[test]
fn ref_and_scope_predicates_keep_ancestors_outside_the_scope() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('[.outside>&]').in(parent('[.shell>&]'))]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    assert!(
        result
            .css
            .contains(&format!(":has(:is(.outside>:where(.{item})))")),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(":is(.shell>:where(.{parent})):has(")),
        "{}",
        result.css
    );
}

#[test]
fn finite_ref_candidates_keep_one_identity_across_all_scope_conditions() {
    let source = SourceFile::new(FileId(1), None, None,
        "import {style} from 'stillwind/react'; const inner=style.ref('inner'); const outer=style.ref('outer'); const first=style.ref('first'); const second=style.ref('second'); const x=style({[choice().in(inner).in(outer)]: 'a'});"
    ).unwrap();
    let start = u32::try_from(source.text().find("choice()").unwrap()).unwrap();
    let facts = [TypeFact {
        start,
        end: start + 6,
        value: ExportedValue::Refs(
            SymbolicValue::finite(
                AssetKind::Ref,
                ["first", "second"].map(|name| {
                    let start = u32::try_from(source.text().find(name).unwrap()).unwrap();
                    IdentityId::new(
                        AssetKind::Ref,
                        ModuleId::from("test.tsx"),
                        start,
                        start + u32::try_from(name.len()).unwrap(),
                        name,
                    )
                }),
            )
            .unwrap(),
        )
        .into(),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@utility a { color:red; } @stillwind-slot utilities;",
    )
    .unwrap();
    let result = Compiler::compile_source_with_type_facts(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &facts,
        &BTreeMap::new(),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let first = marker(&result, "first");
    let second = marker(&result, "second");
    for candidate in [&first, &second] {
        let selector = result
            .css
            .split('{')
            .find(|part| part.contains(candidate))
            .expect("candidate selector");
        assert_eq!(
            selector
                .matches(&format!(":has(:is(:where(.{candidate})"))
                .count(),
            2,
            "{selector}"
        );
        let other = if candidate == &first { &second } else { &first };
        assert!(!selector.contains(other), "{selector}");
    }
}

#[test]
fn registry_resolved_has_and_pseudo_element_conditions_reject_invalid_ref_placement() {
    for (expression, variants) in [
        ("item('has-hover').in(parent)", ""),
        (
            "item('custom-has').in(parent)",
            "@variant custom-has { &:has(.active) { @slot; } }",
        ),
        ("item('before')", ""),
        ("item().in(parent('before'))", ""),
        (
            "item('custom-pseudo')",
            "@variant custom-pseudo { &::before { @slot; } }",
        ),
        (
            "item('nested-has')",
            "@variant nested-has { &:has(:has(.active)) { @slot; } }",
        ),
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref(); const item=style.ref(); const x=style({{[{expression}]: 'a'}});"
            ),
            &format!("{variants} @utility a {{ color:red; }}"),
        );
        assert!(
            result
                .lint_findings
                .iter()
                .any(|error| error.diagnostic == LintDiagnostic::UnavailableVariantPrefix),
            "{expression}: {result:#?}"
        );
        assert!(result.diagnostics.is_empty(), "{expression}: {result:#?}");
        assert!(
            !result.css.contains("color:red"),
            "{expression}: {}",
            result.css
        );
    }
    for expression in ["item('has-hover')", "item().in(parent('has-hover'))"] {
        let (_, valid) = compile_registry(
            &format!(
                "const parent=style.ref(); const item=style.ref(); const x=style({{[{expression}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(valid.lint_findings.is_empty(), "{expression}: {valid:#?}");
        assert!(valid.css.contains(":has(:hover)"), "{}", valid.css);
    }
}

#[test]
fn dynamic_scope_includes_every_candidate_without_runtime_lookup() {
    let (source, result) = compile(
        "const a=style.ref();const b=style.ref();const x=style(style.anchorScope([flag?a:b]));const y=style((flag?a:b).positionAnchor());",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let scope = result
        .css
        .split("anchor-scope:")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(scope.split(',').count(), 2, "{scope}");
    for name in scope.split(',') {
        assert!(result.css.contains(&format!("position-anchor:{name};")));
    }
    let code = result.apply_to_source(&source).unwrap();
    let x = code
        .split("const x=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert!(!x.contains("flag"), "{x}");
    assert!(!result.css.contains(":where("));
}
