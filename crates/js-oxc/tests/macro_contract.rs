//! Public diagnostics for the macro value-expression contract.

use std::collections::BTreeMap;

use stillwind_diagnostics::{CompileErrorKind, LintKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, ModuleId, SourceFile};

#[test]
fn calls_constructors_and_local_computation_need_no_pure_annotation() {
    for value in [
        "compute(input)",
        "compute?.(input)",
        "object.compute(input)",
        "new URL(path)",
        "new Builder(input)",
        "tag`value:${input}`",
        "(() => { const items = []; items.push(input); return items.join(' '); })()",
        "(function () { let n = 0; n++; return n; })()",
        "{ get active() { return input; } }",
        "() => { outside++; }",
        "useful(input)",
        "useTheme()",
        "React.useContext(context)",
        "React['useContext'](context)",
        "use(resource)",
        "useTheme?.()",
        "class { static { run(); } }",
        "<Widget />",
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const props = style({{ class: {value} }});"
        )).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.lint_findings.is_empty(), "{value}: {parsed:#?}");
        assert!(
            parsed.diagnostics.is_empty(),
            "{value}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn visible_mutations_produce_findings_even_in_empty_branches_or_pure_calls() {
    for value in [
        "state++",
        "--state",
        "state = 1",
        "state.x += 2",
        "delete state.x",
        "compute(state++)",
        "compute?.(state++)",
        "new Builder(state++)",
        "/*#__PURE__*/ compute(state++)",
    ] {
        for body in [
            format!("style({{ '--x': {value} }})"),
            format!("style(({value}) ? [] : [])"),
            format!("style({{ class: [{value}] }})"),
        ] {
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                format!("import {{style}} from 'stillwind/react'; const props = {body};"),
            )
            .unwrap();
            let parsed = parse(
                &source,
                &ModuleId::from("test.tsx"),
                &[],
                &BTreeMap::default(),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            );
            let errors = parsed
                .lint_findings
                .iter()
                .filter(|error| error.kind == LintKind::MacroArgumentMutation)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{body}: {:?}", parsed.diagnostics);
            assert!(
                source
                    .slice(errors[0].labels[0].span)
                    .unwrap()
                    .contains("state")
            );
            assert_eq!(errors[0].code(), "macro-argument-mutation");
        }
    }
}

#[test]
fn execution_boundaries_must_be_evaluated_outside_macros() {
    for value in ["await read()", "yield read()", "import('module')"] {
        let source = SourceFile::new(FileId(1), None, None, format!("import {{style}} from 'stillwind/react'; async function* render() {{ return style({{ 'data-value': {value} }}); }}")).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        let errors = parsed
            .lint_findings
            .iter()
            .filter(|error| error.kind == LintKind::MacroExecutionBoundary)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{value}: {:?}", parsed.diagnostics);
        assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), value);
        assert_eq!(errors[0].code(), "macro-execution-boundary");
    }
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; async function render() { const theme = useTheme(); const value = await read(); state++; return style({ '--x': value, class: theme }); }").unwrap();
    assert!(
        parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        )
        .diagnostics
        .is_empty()
    );
}

#[test]
fn keyed_assets_and_macros_inside_function_bodies_are_validated() {
    for body in [
        "const transition = style.viewTransition({old:'opacity-0'}); const props = style(transition.for(index++));",
        "function nested() { return style({'--x': index++}); } const props = style('flex');",
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{style}} from 'stillwind/react'; {body}"),
        )
        .unwrap();
        assert!(
            parse(
                &source,
                &ModuleId::from("test.tsx"),
                &[],
                &BTreeMap::default(),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous
            )
            .lint_findings
            .iter()
            .any(|error| error.kind == LintKind::MacroArgumentMutation)
        );
    }
}

#[test]
fn container_projection_rejects_extra_arguments() {
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; const ref = style.ref(); const props = style(ref.asContainer('size', ignored()));").unwrap();
    assert!(
        parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        )
        .diagnostics
        .iter()
        .any(|error| error.kind == CompileErrorKind::InvalidContainerType)
    );
}

#[test]
fn scope_analysis_probes_receiver_and_every_unresolved_list_entry() {
    let text =
        "import {style} from 'stillwind/react';const x=style(receiver.asAnchor([first,second]));";
    let source = SourceFile::new(FileId(1), None, None, text).unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    for name in ["receiver", "first", "second"] {
        let start = u32::try_from(text.find(name).unwrap()).unwrap();
        assert!(
            parsed.type_probes.iter().any(|span| span.start.0 == start
                && span.end.0 == start + u32::try_from(name.len()).unwrap()),
            "missing {name}: {:?}",
            parsed.type_probes
        );
    }
}
