//! Source lint detection for canonical utility spelling, redundant variants, and
//! untyped variables. Findings retain original source snapshots; Reporter applies
//! severity and suppression.

use std::sync::Arc;

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_diagnostics::{
    FixAction, LintDiagnostic, LintFinding, LintKind, SourceEdit, SourceRange,
};
use stillwind_ir::{
    ClassExpression, GroupEntry, ModuleAnalysis, RuleExpression, StyleInput, StyleVariant,
};
use stillwind_source::{ReplaceText, SourceFile, Span, Spanned};

use stillwind_design_system::ThemeIndex;

use crate::DesignSystemQuery;
use crate::candidate_fixes::{WrittenSource, detect as detect_fixes};
use crate::compiler::check_candidate_name;

/// Returns the source lint findings of the utility literals in `module`'s style inputs.
/// `theme` must belong to the snapshot of `query`.
pub(crate) fn detect(
    source: &SourceFile,
    module: &ModuleAnalysis,
    query: &DesignSystemQuery,
    theme: &ThemeIndex,
) -> Vec<LintFinding> {
    let settings = query.lint_settings();
    let canonical_enabled = settings.enabled_for_detection(LintKind::PreferCanonicalUtilities);
    if ![
        LintKind::PreferCanonicalUtilities,
        LintKind::NoRestrictedUtilities,
        LintKind::RedundantVariant,
        LintKind::PreferTypedVariables,
    ]
    .into_iter()
    .any(|kind| settings.enabled_for_detection(kind))
    {
        return Vec::new();
    }
    let mut literals = Vec::new();
    for call in module.ir.style_inputs() {
        collect_inputs(&call.inputs, "", &mut literals);
    }
    let mut findings = Vec::new();
    for (literal, prefix) in literals {
        let boundaries = literal_boundaries(source, literal);
        let mut offset = 0;
        for token in literal.value.split_whitespace() {
            let Some(relative) = literal.value[offset..].find(token) else {
                continue;
            };
            let start = offset + relative;
            let end = start + token.len();
            offset = end;
            let span = boundaries.as_ref().map_or(literal.span, |boundaries| {
                Span::new(literal.span.file, boundaries[start], boundaries[end])
            });
            findings.extend(check_candidate_name(token, &settings, span));
            let part = |range: std::ops::Range<usize>| {
                let boundaries = boundaries.as_ref()?;
                Some(Span::new(
                    literal.span.file,
                    *boundaries.get(start + range.start)?,
                    *boundaries.get(start + range.end)?,
                ))
            };
            let range = |span| SourceRange::from_source(Arc::clone(&module.source), span).ok();
            findings.extend(detect_fixes(
                token,
                &prefix,
                theme,
                &settings,
                &WrittenSource {
                    span,
                    part: &part,
                    range: &range,
                },
            ));
            if !canonical_enabled || boundaries.is_none() {
                continue;
            }
            let Ok(Some(canonical)) =
                query.canonical_candidate(&format_args!("{prefix}{token}").to_lean_string())
            else {
                continue;
            };
            let Some(replacement) = canonical.strip_prefix(prefix.as_str()) else {
                continue;
            };
            let Ok(range) = SourceRange::from_source(Arc::clone(&module.source), span) else {
                continue;
            };
            let quote = source.text()[..literal.span.start.0 as usize]
                .chars()
                .next_back()
                .unwrap_or('"');
            let replacement = escape_literal(replacement, quote);
            let mut finding = LintFinding::new(LintDiagnostic::NonCanonicalUtility, span);
            finding.actions.push(FixAction {
                title: "Use canonical utility spelling".into(),
                edits: vec![SourceEdit {
                    range,
                    replacement: (replacement).into(),
                }],
            });
            findings.push(finding);
        }
    }
    findings
}

fn collect_inputs<'a>(
    inputs: &'a [StyleInput],
    prefix: &str,
    literals: &mut Vec<(&'a Spanned<LeanStr>, LeanString)>,
) {
    for input in inputs {
        match input {
            StyleInput::Array(inputs) => collect_inputs(inputs, prefix, literals),
            StyleInput::Class(expression) => collect_class(expression, prefix, literals),
            StyleInput::Conditional { branches, .. } => {
                collect_inputs(branches.consequent(), prefix, literals);
                if let Some(alternate) = branches.alternate() {
                    collect_inputs(alternate, prefix, literals);
                }
            }
            _ => {}
        }
    }
}

fn collect_class<'a>(
    expression: &'a ClassExpression,
    prefix: &str,
    literals: &mut Vec<(&'a Spanned<LeanStr>, LeanString)>,
) {
    match expression {
        ClassExpression::Rule(rule) => collect_rule(rule, prefix, literals),
        ClassExpression::Array(elements) => {
            for element in elements {
                collect_class(element, prefix, literals);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            for expression in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_class(expression, prefix, literals);
            }
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                if let GroupEntry::Class(expression) = entry {
                    collect_class(expression, prefix, literals);
                }
            }
        }
        ClassExpression::ClassValue { .. } => {}
    }
}

fn collect_rule<'a>(
    expression: &'a RuleExpression,
    prefix: &str,
    literals: &mut Vec<(&'a Spanned<LeanStr>, LeanString)>,
) {
    match expression {
        RuleExpression::Utilities(value) => literals.push((value, prefix.to_lean_string())),
        RuleExpression::Array(elements)
        | RuleExpression::Group {
            inputs: elements, ..
        } => {
            for element in elements {
                collect_rule(element, prefix, literals);
            }
        }
        RuleExpression::Conditional { branches, .. } => {
            for expression in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_rule(expression, prefix, literals);
            }
        }
        RuleExpression::Variant { variant, inputs } => {
            let prefix = match variant {
                StyleVariant::Prefix(value) => {
                    format_args!("{prefix}{}", value.value).to_lean_string()
                }
                _ => prefix.to_lean_string(),
            };
            for input in inputs {
                collect_rule(input, &prefix, literals);
            }
        }
        _ => {}
    }
}

fn escape_literal(value: &str, quote: char) -> LeanString {
    let quote = if matches!(quote, '\'' | '"' | '`') {
        quote
    } else {
        '`'
    };
    let mut value = value
        .replace_char('\\', "\\\\")
        .replace_char(quote, &format_args!("\\{quote}").to_lean_string());
    if quote == '`' {
        value = value.replace_text("${", "\\${");
    }
    value
}

/// Maps decoded byte boundaries back to a contiguous original literal. Unmapped projections are skipped.
pub(crate) fn literal_boundaries(
    source: &SourceFile,
    literal: &Spanned<LeanStr>,
) -> Option<Vec<u32>> {
    let raw = source.slice(literal.span).ok()?;
    let mut text = LeanString::new();
    let mut positions = vec![literal.span.start.0];
    let mut index = 0;
    while index < raw.len() {
        let character = raw[index..].chars().next()?;
        index += character.len_utf8();
        let value = if character == '\\' {
            let escape = raw[index..].chars().next()?;
            index += escape.len_utf8();
            match escape {
                '\n' | '\r' => {
                    if escape == '\r' && raw[index..].starts_with('\n') {
                        index += 1;
                    }
                    *positions.last_mut()? = literal
                        .span
                        .start
                        .0
                        .checked_add(u32::try_from(index).ok()?)?;
                    continue;
                }
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'v' => '\u{000b}',
                '0' => '\0',
                'x' | 'u' => {
                    let brace = escape == 'u' && raw[index..].starts_with('{');
                    let end = if brace {
                        index + raw[index..].find('}')?
                    } else {
                        index + if escape == 'x' { 2 } else { 4 }
                    };
                    let mut value =
                        u32::from_str_radix(raw.get(index + usize::from(brace)..end)?, 16).ok()?;
                    index = end + usize::from(brace);
                    if !brace && (0xd800..=0xdbff).contains(&value) {
                        let tail = raw.get(index..)?.strip_prefix("\\u")?;
                        let low = u32::from_str_radix(tail.get(..4)?, 16).ok()?;
                        if !(0xdc00..=0xdfff).contains(&low) {
                            return None;
                        }
                        value = 0x10000 + ((value - 0xd800) << 10) + low - 0xdc00;
                        index += 6;
                    }
                    char::from_u32(value)?
                }
                other => other,
            }
        } else if character == '\r' {
            if raw[index..].starts_with('\n') {
                index += 1;
            }
            '\n'
        } else {
            character
        };
        text.push(value);
        let boundary = literal
            .span
            .start
            .0
            .checked_add(u32::try_from(index).ok()?)?;
        positions.extend(std::iter::repeat_n(boundary, value.len_utf8()));
    }
    (text == literal.value).then_some(positions)
}
