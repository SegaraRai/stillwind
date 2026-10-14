//! Rewrites runtime conditions in a candidate to their canonical spelling.
//!
//! The canonical spelling has at most one `!` and no empty comparison. Rewriting keeps
//! every other byte of the candidate, so the utility and its other variants are
//! unchanged.

use stillwind_css::find_css_functions;
use stillwind_design_system::{RuntimeCondition, ThemeIndex, VariantApplication, resolve_variant};
use stillwind_syntax::parse_candidate;

/// Returns `raw` with canonical runtime conditions, or `None` when every condition is
/// already canonical or the candidate does not parse.
///
/// Captures of conditional variants and the first argument of `--if()` functions are
/// rewritten. In arbitrary values, `_` encodes a space, so the condition is decoded
/// before parsing and the canonical text encodes spaces as `_` again.
pub(super) fn canonical_conditions(raw: &str, theme: &ThemeIndex) -> Option<String> {
    let candidate = parse_candidate(raw)?;
    let mut rewritten = raw.to_owned();
    for variant in &candidate.variants {
        let Some(VariantApplication::Conditional(canonical)) = resolve_variant(variant, theme)
        else {
            continue;
        };
        let Some(open) = variant.raw.find('(') else {
            continue;
        };
        let Some(capture) = variant.raw[open + 1..].strip_suffix(')') else {
            continue;
        };
        if capture.trim() != canonical {
            let replacement = format!("{}({canonical})", &variant.raw[..open]);
            rewritten =
                rewritten.replacen(&format!("{}:", variant.raw), &format!("{replacement}:"), 1);
        }
    }
    let functions = find_css_functions(&rewritten, &["--if"], 0);
    for function in functions.into_iter().rev() {
        let arguments = &rewritten[function.arguments.clone()];
        let end = top_level_comma(arguments).unwrap_or(arguments.len());
        let written = &arguments[..end];
        let decoded = written.replace('_', " ");
        let Some(condition) = RuntimeCondition::parse(&decoded) else {
            continue;
        };
        let canonical = condition.to_string().replace(' ', "_");
        if written.trim() != canonical {
            let start = function.arguments.start;
            rewritten.replace_range(start..start + end, &canonical);
        }
    }
    (rewritten != raw).then_some(rewritten)
}

/// Returns the byte offset of the first comma outside nested parentheses and brackets.
fn top_level_comma(arguments: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, byte) in arguments.bytes().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}
