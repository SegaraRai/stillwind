//! Diagnostic source ranges and motion identities during span mapping.

use std::collections::BTreeMap;

use stillwind_diagnostics::LintFinding;
use stillwind_ir::{ClassExpression, RuleExpression, ScriptLanguage, ScriptModuleKind, StyleInput};
use stillwind_js_oxc::parse;
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact};
use stillwind_source::{FileId, ModuleId, SourceError, SourceFile, Span};

use super::compile;

#[test]
fn escaped_utility_diagnostics_keep_the_literal_provenance() {
    let (source, result) = compile(r"const x = style('\u006eot-a-utility');", "");
    assert_ne!(result.lint_findings, [] as [LintFinding; 0]);
    let spans = result
        .lint_findings
        .iter()
        .flat_map(|d| d.labels.iter())
        .map(|label| source.slice(label.span).unwrap())
        .collect::<Vec<_>>();
    assert!(spans.contains(&r"\u006eot-a-utility"), "{spans:?}");
}

#[test]
fn external_motion_identity_does_not_become_a_local_source_span() {
    let source = SourceFile::new(FileId(1), None, None, "import { style } from 'stillwind/react'; import { motion } from './producer'; const x = style(motion(''));").unwrap();
    let start = u32::try_from(source.text().rfind("motion(").unwrap()).unwrap();
    let identity = IdentityId::new(
        AssetKind::Keyframes,
        ModuleId::from("producer.tsx"),
        4000,
        4006,
        "motion",
    );
    let facts = [TypeFact {
        start,
        end: start + 6,
        value: ExportedValue::Keyframes(SymbolicValue::exact(identity.clone())).into(),
    }];
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &facts,
        &BTreeMap::new(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let mapped = parsed
        .module
        .try_map_spans(|span| {
            source.slice(span)?;
            Ok::<_, SourceError>(span)
        })
        .unwrap();
    let StyleInput::Class(ClassExpression::Rule(rule)) = &mapped.style_calls[0].inputs[0] else {
        panic!("expected static keyframes");
    };
    let RuleExpression::KeyframesUse { keyframes, .. } = rule.as_ref() else {
        panic!("keyframes")
    };
    assert_eq!(keyframes.identity, identity);
    assert_eq!(source.slice(keyframes.span).unwrap(), "motion");
}

#[test]
fn local_motion_identity_stays_original_while_analysis_spans_are_mapped() {
    let source = SourceFile::new(FileId(1), None, None, "import { style } from 'stillwind/react'; const a=style.keyframes({from:'x'}); const x=style(a(''));").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let mapped = parsed
        .module
        .try_map_spans(|span| Ok::<_, ()>(Span::new(span.file, span.start.0 + 20, span.end.0 + 20)))
        .unwrap();
    let StyleInput::Class(ClassExpression::Rule(rule)) = &mapped.style_calls[0].inputs[0] else {
        panic!("expected keyframes");
    };
    let RuleExpression::KeyframesUse { keyframes, .. } = rule.as_ref() else {
        panic!("keyframes")
    };
    assert_eq!(
        keyframes.identity.start(),
        parsed.module.keyframes_declarations[0].binding.span.start.0
    );
    assert_eq!(
        keyframes.identity.end(),
        parsed.module.keyframes_declarations[0].binding.span.end.0
    );
    let StyleInput::Class(ClassExpression::Rule(rule)) = &parsed.module.style_calls[0].inputs[0]
    else {
        panic!("keyframes")
    };
    let RuleExpression::KeyframesUse {
        keyframes: original,
        ..
    } = rule.as_ref()
    else {
        panic!("keyframes")
    };
    assert_eq!(keyframes.span.start.0, original.span.start.0 + 20);
}
