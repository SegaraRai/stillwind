//! Public diagnostics for the macro value-expression contract.

use stillwind_diagnostics::CompileErrorKind;
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, SourceFile};

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
    ] {
        let source = SourceFile::new(FileId(1), "contract.tsx", format!(
            "import {{style}} from 'stillwind/react'; const props = style({{ class: {value} }});"
        )).unwrap();
        let parsed = parse(&source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{value}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn visible_mutations_are_rejected_even_in_empty_branches_or_pure_calls() {
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
                "contract.tsx",
                format!("import {{style}} from 'stillwind/react'; const props = {body};"),
            )
            .unwrap();
            let parsed = parse(&source);
            let errors = parsed
                .diagnostics
                .iter()
                .filter(|error| error.kind == CompileErrorKind::MacroArgumentMutation)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{body}: {:?}", parsed.diagnostics);
            assert!(
                source
                    .slice(errors[0].labels[0].span)
                    .unwrap()
                    .contains("state")
            );
            assert_eq!(errors[0].code(), "SHMACRO001");
        }
    }
}

#[test]
fn execution_boundaries_must_be_evaluated_outside_macros() {
    for value in [
        "await read()",
        "yield read()",
        "useTheme()",
        "React.useContext(context)",
        "React['useContext'](context)",
        "use(resource)",
        "useTheme?.()",
        "import('module')",
        "class { static { run(); } }",
        "<Widget />",
    ] {
        let source = SourceFile::new(FileId(1), "contract.tsx", format!("import {{style}} from 'stillwind/react'; async function* render() {{ return style({{ 'data-value': {value} }}); }}")).unwrap();
        let parsed = parse(&source);
        let errors = parsed
            .diagnostics
            .iter()
            .filter(|error| error.kind == CompileErrorKind::MacroExecutionBoundary)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{value}: {:?}", parsed.diagnostics);
        assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), value);
        assert_eq!(errors[0].code(), "SHMACRO002");
    }
    let source = SourceFile::new(FileId(1), "contract.tsx", "import {style} from 'stillwind/react'; async function render() { const theme = useTheme(); const value = await read(); state++; return style({ '--x': value, class: theme }); }").unwrap();
    assert!(parse(&source).diagnostics.is_empty());
}

#[test]
fn keyed_assets_and_macros_inside_function_bodies_are_validated() {
    for body in [
        "const transition = style.viewTransition({old:'opacity-0'}); const props = style(transition.for(index++));",
        "function nested() { return style({'--x': index++}); } const props = style('flex');",
    ] {
        let source = SourceFile::new(
            FileId(1),
            "contract.tsx",
            format!("import {{style}} from 'stillwind/react'; {body}"),
        )
        .unwrap();
        assert!(
            parse(&source)
                .diagnostics
                .iter()
                .any(|error| error.kind == CompileErrorKind::MacroArgumentMutation)
        );
    }
}

#[test]
fn container_projection_rejects_extra_arguments() {
    let source = SourceFile::new(FileId(1), "contract.tsx", "import {style} from 'stillwind/react'; const ref = style.ref(); const props = style(ref.container('size', ignored()));").unwrap();
    assert!(
        parse(&source)
            .diagnostics
            .iter()
            .any(|error| error.kind == CompileErrorKind::InvalidContainerType)
    );
}
