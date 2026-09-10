use stillwind_compiler::CompileResult;
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

/// Compare utilities against upstream fixtures using Stillwind's own entrypoints.
/// Tailwind package names are deliberately not aliases in the production loader.
pub(super) fn stillwind_fixture_css(css: &str) -> String {
    let css = css
        .replace("tailwindcss/theme", "stillwind/theme.css")
        .replace("tailwindcss/preflight", "stillwind/preflight.css")
        .replace("tailwindcss/utilities", "stillwind/utilities.css")
        .replace("\"tailwindcss\"", "\"stillwind\"");
    format!("{css}\n@import \"stillwind/builtins.css\";")
}

pub(super) fn fixture_candidates(fixture: &str, case_name: &str) -> String {
    let case_start = fixture
        .find(&format!("## `{case_name}`"))
        .expect("fixture case must exist");
    let candidates_start = fixture[case_start..]
        .find("### Candidates\n\n```text\n")
        .expect("fixture candidates must exist")
        + case_start
        + "### Candidates\n\n```text\n".len();
    fixture[candidates_start..]
        .split_once("\n```")
        .expect("fixture candidates must close")
        .0
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn fixture_code_block(section: &str, heading: &str, language: &str) -> Option<String> {
    let marker = format!("### {heading}\n\n```{language}\n");
    let start = section.find(&marker)? + marker.len();
    Some(section[start..].split_once("\n```")?.0.trim().to_owned())
}

pub(super) fn fixture_case_for_candidate<'a>(fixture: &'a str, candidate: &str) -> &'a str {
    fixture
        .split("## `")
        .skip(1)
        .find(|section| {
            fixture_code_block(section, "Candidates", "text").is_some_and(|candidates| {
                candidates
                    .split_whitespace()
                    .any(|value| value == candidate)
            })
        })
        .unwrap_or_else(|| panic!("fixture case for `{candidate}` must exist"))
}

pub(super) fn fixture_case_named<'a>(fixture: &'a str, name: &str) -> &'a str {
    fixture
        .split("## `")
        .skip(1)
        .find(|section| section.starts_with(&format!("{name}`")))
        .unwrap_or_else(|| panic!("fixture case `{name}` must exist"))
}

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
    Compiler::compile_source(
        &OxcSourceAnalyzer,
        &SourceFile::new(
            FileId(91),
            "src/Property.tsx",
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style({utility:?});"
            ),
        )
        .unwrap(),
        &crate::css_fixture::source(
            FileId(92),
            "stillwind.css",
            "@theme { --spacing: 0.25rem; }",
        )
        .unwrap(),
    )
}

pub(super) fn tailwind_class_selector(candidate: &str) -> String {
    candidate
        .chars()
        .flat_map(|character| match character {
            '/' | '[' | ']' | ':' | '!' | '(' | ')' | '\'' | '"' | '=' | '@' => ['\\', character],
            character => ['\0', character],
        })
        .filter(|character| *character != '\0')
        .collect()
}

pub(super) fn minify_css(css: &str) -> String {
    let sheet = StyleSheet::parse(css, ParserOptions::default()).expect("CSS must parse");
    sheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .expect("CSS must serialize")
        .code
}
