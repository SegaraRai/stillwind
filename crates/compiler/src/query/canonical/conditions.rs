//! Rewrites runtime conditions in a candidate to their canonical spelling.
//!
//! The canonical spelling has at most one `!` and no empty comparison. Rewriting keeps
//! every other byte of the candidate, so the utility and its other variants are
//! unchanged.

use cssparser::Token;
use lean_string::{LeanString, ToLeanString};

use stillwind_css::{find_css_functions, split_once_top_level};
use stillwind_design_system::{RuntimeCondition, ThemeIndex, VariantApplication, resolve_variant};
use stillwind_source::ReplaceText;
use stillwind_syntax::parse_candidate;

/// Returns `raw` with canonical runtime conditions, or `None` when every condition is
/// already canonical or the candidate does not parse.
///
/// Captures of conditional variants and the first argument of `--if()` functions are
/// rewritten. In arbitrary values, `_` encodes a space, so the condition is decoded
/// before parsing and the canonical text encodes spaces as `_` again.
pub(super) fn canonical_conditions(raw: &str, theme: &ThemeIndex) -> Option<LeanString> {
    let candidate = parse_candidate(raw)?;
    let mut rewritten = raw.to_lean_string();
    for variant in &candidate.variants {
        let Some(VariantApplication::Conditional(canonical)) = resolve_variant(variant, theme).ok()
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
            let replacement =
                format_args!("{}({canonical})", &variant.raw[..open]).to_lean_string();
            rewritten = rewritten
                .replacen(
                    format_args!("{}:", variant.raw).to_lean_string().as_str(),
                    &format_args!("{replacement}:").to_lean_string(),
                    1,
                )
                .into();
        }
    }
    let functions = find_css_functions(&rewritten, &["--if"], 0);
    for function in functions.into_iter().rev() {
        let arguments = &rewritten[function.arguments.clone()];
        let (written, _) = split_once_top_level(arguments, |token| matches!(token, Token::Comma));
        let end = written.len();
        let decoded = written.replace_char('_', " ");
        let Some(condition) = RuntimeCondition::parse(&decoded) else {
            continue;
        };
        let canonical = condition.to_lean_string().replace_char(' ', "_");
        if written.trim() != canonical {
            let start = function.arguments.start;
            rewritten.replace_range(start..start + end, &canonical);
        }
    }
    (rewritten != raw).then_some(rewritten)
}
