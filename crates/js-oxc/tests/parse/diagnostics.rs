//! Checks parser failures and transactional IR span mapping.

use super::*;

#[test]
fn parser_diagnostics_keep_external_detail_as_notes() {
    let source = SourceFile::new(FileId(1), None, None, "const = ;").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let error = parsed
        .diagnostics
        .iter()
        .find(|error| error.diagnostic == CompileDiagnostic::JavaScriptParse)
        .unwrap();
    assert_eq!(error.diagnostic, CompileDiagnostic::JavaScriptParse);
    assert_eq!(error.to_string(), "JavaScript parse error");
    assert_eq!(error.labels[0].span, source.full_span());
    assert_ne!(error.notes[0], "");
    assert_eq!(parsed.lint_findings, [] as [LintFinding; 0]);
}

/// IR span mapping visits every span and fails atomically.
///
/// A projection that fails partway must leave the original IR untouched, so callers can
/// retry with another map without repairing state.
#[test]
fn ir_span_mapping_relocates_nested_expressions_transactionally() {
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; const ref = style.ref('target'); const props = style(ref, active ? 'flex' : 'grid', {class: external, 'aria-label': label});").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let before = parsed.module.clone();
    let mut visited = 0;
    let relocated = before
        .try_map_spans(|span| {
            visited += 1;
            source.slice(span).unwrap();
            Ok::<_, ()>(Span::new(FileId(2), span.start.0 + 17, span.end.0 + 17))
        })
        .unwrap();
    assert!(visited > 10);
    let restored = relocated
        .try_map_spans(|span| Ok::<_, ()>(Span::new(FileId(1), span.start.0 - 17, span.end.0 - 17)))
        .unwrap();
    assert_eq!(restored, before);
    let mut visited = 0;
    assert!(
        before
            .try_map_spans(|span| {
                visited += 1;
                if visited == 3 {
                    Err("ambiguous projection")
                } else {
                    Ok(span)
                }
            })
            .is_err()
    );
    assert_eq!(parsed.module, before);
}
