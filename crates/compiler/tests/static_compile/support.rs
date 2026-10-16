//! Compilation and stylesheet comparisons for the static-compile suite.
//!
//! Rule comparisons retain condition contexts and generation order. The Tailwind
//! parity oracle normalizes arithmetic and serializes rules with Lightning CSS.
//! Fixed-theme cases share snapshots while each compilation analyzes fresh source.

use std::sync::{Arc, OnceLock};

use stillwind_compiler::{
    CompileResult, CompilerOptions, Identifiers, ModuleCompiler, StylesheetRenderer,
};
use stillwind_css::fold_calc_expressions;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

pub(super) fn css_rule(css: &str, class_name: &str) -> String {
    let selector = format!(".{class_name}");
    let start = css
        .find(&selector)
        .unwrap_or_else(|| panic!("CSS rule `{selector}` must exist"));
    let open = css[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("CSS rule must open");
    let mut depth = 0_u32;
    for (offset, character) in css[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return css[start..=open + offset].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("CSS rule must close");
}

pub(super) fn normalize_css(css: &str) -> String {
    css.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn skip_css_trivia(css: &str, mut cursor: usize) -> usize {
    loop {
        while css[cursor..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            cursor += css[cursor..].chars().next().unwrap().len_utf8();
        }
        if css[cursor..].starts_with("/*")
            && let Some(end) = css[cursor + 2..].find("*/")
        {
            cursor += end + 4;
            continue;
        }
        return cursor;
    }
}

pub(super) fn closing_css_brace(css: &str, open: usize) -> usize {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in css[open..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            }
            _ => {}
        }
    }
    panic!("CSS block must close");
}

pub(super) fn collect_class_rule_signatures(
    css: &str,
    contexts: &mut Vec<String>,
    signatures: &mut Vec<String>,
) {
    let mut cursor = 0;
    while cursor < css.len() {
        cursor = skip_css_trivia(css, cursor);
        if cursor >= css.len() {
            break;
        }
        let semicolon = css[cursor..].find(';').map(|offset| cursor + offset);
        let open = css[cursor..].find('{').map(|offset| cursor + offset);
        if semicolon.is_some_and(|semicolon| open.is_none_or(|open| semicolon < open)) {
            cursor = semicolon.unwrap() + 1;
            continue;
        }
        let Some(open) = open else {
            break;
        };
        let close = closing_css_brace(css, open);
        let header = css[cursor..open].trim();
        let body = &css[open + 1..close];
        if header.starts_with('@') {
            let is_layer = header.starts_with("@layer ");
            if !is_layer {
                contexts.push(normalize_css(header));
            }
            collect_class_rule_signatures(body, contexts, signatures);
            if !is_layer {
                contexts.pop();
            }
        } else if header.contains(".CLASS") {
            signatures.push(format!(
                "{}|{}|{}",
                contexts.join(">"),
                normalize_css(header),
                normalize_css(body)
            ));
        }
        cursor = close + 1;
    }
}

pub(super) fn class_rule_signatures(css: &str) -> Vec<String> {
    let mut signatures = Vec::new();
    collect_class_rule_signatures(css, &mut Vec::new(), &mut signatures);
    signatures
}

pub(super) fn compile_one_utility(utility: &str) -> CompileResult {
    static SNAPSHOT: OnceLock<DesignSystemSnapshot> = OnceLock::new();

    let snapshot = SNAPSHOT.get_or_init(|| {
        DesignSystemSnapshot::from_source(
            &css_source(FileId(92), "@theme { --spacing: 0.25rem; }").unwrap(),
            CssSourceName::from("stillwind.css"),
            CompilerOptions::default().design_system(),
        )
        .unwrap()
    });
    let source = SourceFile::new(
        FileId(91),
        None,
        None,
        format!("import {{ style }} from \"stillwind/react\"; const props = style({utility:?});"),
    )
    .unwrap();
    compile_with_snapshot(&source, snapshot, "test.tsx", ScriptLanguage::Tsx)
}

pub(super) fn compile_with_snapshot(
    source: &SourceFile,
    snapshot: &DesignSystemSnapshot,
    module_id: &str,
    language: ScriptLanguage,
) -> CompileResult {
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(source.clone()),
            ModuleId::from(module_id),
            language,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let options = CompilerOptions::default();
    let module = ModuleCompiler::new(snapshot.clone(), options.clone())
        .compile(&analysis, &Identifiers::default());
    StylesheetRenderer::new(snapshot, &options, &Identifiers::default())
        .render_compilation(&module, source.full_span())
}

pub(super) fn tailwind_class_selector(candidate: &str) -> String {
    candidate
        .chars()
        .flat_map(|character| match character {
            '/' | '[' | ']' | ':' | '!' | '(' | ')' | '\'' | '"' | '=' | '@' | '&' | '>' | '.' => {
                ['\\', character]
            }
            character => ['\0', character],
        })
        .filter(|character| *character != '\0')
        .collect()
}

pub(super) fn minify_css(css: &str) -> String {
    // Value normalization belongs to the test oracle, never the production emitter.
    let css = fold_calc_expressions(css, None);
    let sheet = StyleSheet::parse(&css, ParserOptions::default()).expect("CSS must parse");
    sheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .expect("CSS must serialize")
        .code
}
