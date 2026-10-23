//! Checks macro binding, import editing, and semantic reads.

use super::*;

/// Aliased macro imports are recognized, and a shadowing local is not.
#[test]
fn framework_registry_preserves_import_aliases_and_shadowing() {
    for spec in FRAMEWORKS {
        let source = SourceFile::new(
            FileId(1), None, None,
            format!(
                "import {{style as s}} from '{}'; const a = s('flex'); function f(s: Function) {{ return s('grid'); }}",
                spec.macro_import,
            ),
        ).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.module.style_calls[0].output, spec.output);
        assert_eq!(parsed.module.style_calls.len(), 1);
    }
}

/// A type-only import never selects an adapter or expands calls.
#[test]
fn type_only_macro_imports_do_not_select_a_framework_or_expand_calls() {
    for declaration in [
        "import type {style as s} from 'stillwind/react';",
        "import {type style as s} from 'stillwind/react';",
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("{declaration} const a = s('grid');"),
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
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(parsed.module.style_calls, [] as [StyleCall; 0]);

        let mixed = SourceFile::new(
            FileId(2),
            None,
            None,
            format!(
                "{declaration} import {{style}} from 'stillwind/solid'; const b = style('flex');"
            ),
        )
        .unwrap();
        let parsed = parse(
            &mixed,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(
            parsed.module.style_calls[0].output,
            AdapterKind::Solid.spec().output
        );
        assert_eq!(parsed.module.style_calls.len(), 1);
    }
}

/// Macro specifier spans, references and reserved names stay usable for edits.
///
/// The reference list must contain only real uses of the macro binding, so a host edit
/// can remove the import without touching unrelated code.
#[test]
fn import_edit_metadata_tracks_macro_references_and_preserves_mixed_bindings() {
    let source = SourceFile::new(FileId(1), None, None, "import fallback, {style as s, type StyleArgument, other} from 'stillwind/react'; const a = s('flex'); const retained = s; function f(s: Function, stillwindRef: string) { return s('grid') + stillwindClsx; }").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let imports = &parsed.module.imports;
    assert_eq!(imports.macros.len(), 1);
    let declaration = &imports.macros[0];
    assert!(
        source
            .slice(declaration.span)
            .unwrap()
            .starts_with("import fallback")
    );
    assert_eq!(declaration.specifiers.len(), 4);
    let macros: Vec<_> = declaration
        .specifiers
        .iter()
        .filter(|specifier| specifier.is_macro)
        .collect();
    assert_eq!(macros.len(), 1);
    assert_eq!(macros[0].local, "s");
    assert_eq!(source.slice(macros[0].span).unwrap(), "style as s");
    assert_eq!(macros[0].references.len(), 2);
    for reference in &macros[0].references {
        assert_eq!(source.slice(*reference).unwrap(), "s");
    }
    for reserved in ["fallback", "StyleArgument", "stillwindRef", "stillwindClsx"] {
        assert!(imports.reserved_names.iter().any(|name| name == reserved));
    }
}

/// Specifier spans cover the literal including its quotes, even after non-ASCII text.
#[test]
fn module_specifier_spans_preserve_quotes_and_utf8_offsets() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "// 日本語 🧭\nimport {x} from '@a'; export {x as y} from \"@b\"; export * from '@c';",
    )
    .unwrap();
    let summary = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    )
    .summary;
    assert_eq!(summary.module_specifiers.len(), 3);
    for (specifier, literal) in summary
        .module_specifiers
        .iter()
        .zip(["'@a'", "\"@b\"", "'@c'"])
    {
        assert_eq!(
            &source.text()[specifier.start as usize..specifier.end as usize],
            literal
        );
    }
}

/// Each resolved macro binding selects the format of its own calls.
#[test]
fn macro_bindings_keep_independent_output_formats() {
    let source = SourceFile::new(FileId(1), None, None, "import type { StyleArgument } from 'stillwind/react'; import { style } from 'stillwind/solid'; const props = style('flex');").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(
        parsed.module.style_calls[0].output,
        AdapterKind::Solid.spec().output
    );
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let mixed = SourceFile::new(FileId(2), None, None, "import {style} from 'stillwind/solid'; import {style as reactStyle} from 'stillwind/react'; const props = style('flex'); const other = reactStyle('grid');").unwrap();
    let parsed = parse(
        &mixed,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.diagnostics, []);
    assert_eq!(
        parsed
            .module
            .style_calls
            .iter()
            .map(|call| call.output)
            .collect::<Vec<_>>(),
        vec![
            AdapterKind::Solid.spec().output,
            AdapterKind::React.spec().output
        ]
    );
}

/// Import slices follow local aliases and drop unreachable imports.
///
/// Both `probe_imports` and per-export slices must reach through a reassigned local,
/// while an import nothing reads stays out of the result.
#[test]
fn semantic_reads_follow_local_aliases_without_unrelated_imports() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"
import { style } from 'stillwind/react';
import { first } from './first';
import { second } from './second';
import { unrelated } from './unrelated';
let selected = first;
selected = second;
const alias = selected;
const unused = unrelated;
export const attached = style(alias);
",
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
    assert_eq!(
        parsed
            .summary
            .probe_imports
            .iter()
            .map(|import| import.source.as_str())
            .collect::<Vec<_>>(),
        vec!["./first", "./second"]
    );
    assert_eq!(parsed.summary.export_imports["attached"].len(), 2);
}

/// A parameter that shadows an import contributes no probe import.
#[test]
fn semantic_reads_preserve_binding_shadowing() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"
import { style } from 'stillwind/react';
import { handle } from './unrelated';
export function attach(handle: unknown) { return style(handle); }
",
    )
    .unwrap();
    assert_eq!(
        parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        )
        .summary
        .probe_imports,
        [] as [ImportBinding; 0]
    );
}
