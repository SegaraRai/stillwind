//! Lowers experimental runtime conditionals after declaration ordering and formatting.
//!
//! Helpers are declared beside their consumers, so inherited computed helper values
//! cannot freeze a child's condition. Variant callers keep guarded declarations in
//! separate rules: `revert-rule` removes the whole current rule for that property.
//! Use [`guard`] while constructing variant rules and [`lower`] after ordering
//! declarations. Rule boundaries are finalized by the owner before publication. This
//! module emits canonical helper names; prefix rewriting and source-map attribution
//! belong to its callers.

use std::fmt::Write as _;
use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::find_css_functions;
use stillwind_design_system::{RuntimeCondition, runtime_if_arguments};
use stillwind_diagnostics::{
    CompileDiagnostic, LintDiagnostic, LintFinding, LintKind, LintSettings,
};
use stillwind_source::Span;

use crate::identifiers::{ConditionContext, ConditionRole, IdentifierContext};

/// Writes one helper custom property, or reports why it could not be named.
///
/// Helpers are generated rather than authored, so a host that names them is asked here,
/// where the CSS text they serve is still known.
fn helper(
    identifiers: &IdentifierContext,
    role: ConditionRole,
    text: &str,
) -> Result<LeanString, CompileDiagnostic> {
    Ok(identifiers
        .condition_helper(&ConditionContext {
            role,
            text: text.to_lean_str(),
        })?
        .property())
}

/// A declaration that [`lower`] leaves out because its `--if()` cannot be lowered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SkippedConditional {
    /// Property name of the declaration, trimmed.
    pub(crate) property: LeanStr,
    /// Why the declaration cannot be lowered.
    pub(crate) detail: &'static str,
}

/// CSS after runtime conditional lowering, with the declarations that lowering left
/// out.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Lowered {
    /// CSS whose `--if()` functions are lowered.
    pub(crate) css: LeanString,
    /// Declarations removed from `css`, in source order.
    pub(crate) skipped: Vec<SkippedConditional>,
}

/// Why lowering stops for one declaration.
enum DeclarationFailure {
    /// The declaration is left out for this reason.
    Skip(&'static str),
    /// The host generator failed to name a helper, so the compilation fails.
    Name(CompileDiagnostic),
}

impl From<CompileDiagnostic> for DeclarationFailure {
    fn from(diagnostic: CompileDiagnostic) -> Self {
        Self::Name(diagnostic)
    }
}

/// Lowers `--if(C,Y,N)` in declaration bodies; an odd number of leading `!` swaps the
/// branches. `use_css_if` selects native CSS `if()`; false emits local fallback helpers
/// under the generated custom-property names. Selectors, priorities, strings, and URLs
/// are preserved.
///
/// A declaration that cannot be lowered is removed, and its semicolon remains. That
/// happens for a malformed expression, more than 128 invocations in the declaration, or
/// a CSS-wide branch stored in a fallback helper. That branch is Y for a positive
/// condition and N for a negated one. Negated variants and `--name:value` comparisons
/// require native mode. [`Lowered::skipped`] lists the removed declarations.
///
/// # Errors
///
/// Returns the host generator's failure for a helper's name.
pub(crate) fn lower(
    css: &str,
    use_css_if: bool,
    identifiers: &IdentifierContext,
) -> Result<Lowered, CompileDiagnostic> {
    if !css.contains("--if(") {
        return Ok(Lowered {
            css: css.to_lean_string(),
            skipped: Vec::new(),
        });
    }
    let mut skipped = Vec::new();
    let css = map_declarations(css, |property, value| {
        match lower_declaration(property, value, use_css_if, identifiers) {
            Ok(declaration) => Ok(declaration),
            Err(DeclarationFailure::Skip(detail)) => {
                skipped.push(SkippedConditional {
                    property: property.trim().to_lean_str(),
                    detail,
                });
                Ok(LeanString::new())
            }
            Err(DeclarationFailure::Name(diagnostic)) => Err(diagnostic),
        }
    })?;
    Ok(Lowered { css, skipped })
}

/// Reports each declaration in `skipped` as `invalid-runtime-condition` at `span` when
/// `lint_settings` enables the rule.
pub(super) fn report_skipped_conditionals(
    skipped: Vec<SkippedConditional>,
    span: Span,
    lint_settings: &LintSettings,
    lint_findings: &mut Vec<LintFinding>,
) {
    if !lint_settings.enabled_for_detection(LintKind::InvalidRuntimeCondition) {
        return;
    }
    lint_findings.extend(skipped.into_iter().map(|skipped| {
        LintFinding::new(
            LintDiagnostic::InvalidRuntimeCondition {
                property: skipped.property,
                detail: skipped.detail.into(),
            },
            span,
        )
    }));
}

/// Lowers the `--if()` functions of one declaration and returns its replacement text.
fn lower_declaration(
    property: &str,
    value: &str,
    use_css_if: bool,
    identifiers: &IdentifierContext,
) -> Result<LeanString, DeclarationFailure> {
    let mut value = value.to_lean_string();
    let mut helpers = LeanString::new();
    // Reverse source order keeps the start offsets of pending replacements valid.
    let functions = find_css_functions(&value, &["--if"], 0);
    if functions.len() > MAX_FUNCTIONS {
        return Err(DeclarationFailure::Skip(TOO_MANY_FUNCTIONS));
    }
    // Nested replacements alter outer end offsets, so retain a running edit delta.
    let mut changes: Vec<(usize, isize)> = Vec::new();
    for function in functions.into_iter().rev() {
        let Some(original_end) = function.end else {
            return Err(DeclarationFailure::Skip("unclosed --if()"));
        };
        let shift: isize = changes
            .iter()
            .filter(|(start, _)| *start < original_end)
            .map(|(_, delta)| delta)
            .sum();
        let end = original_end.saturating_add_signed(shift);
        let arguments_end = end - 1;
        let Some((condition, yes, no)) =
            runtime_if_arguments(&value[function.arguments.start..arguments_end])
        else {
            return Err(DeclarationFailure::Skip(
                "expected --if(--condition, true-value, false-value)",
            ));
        };
        if !use_css_if && condition.value.is_some() {
            return Err(DeclarationFailure::Skip(VALUE_COMPARISON));
        }
        let (yes, no) = if condition.negated {
            (no, yes)
        } else {
            (yes, no)
        };
        let condition = RuntimeCondition {
            negated: false,
            ..condition
        };
        let replacement = if use_css_if {
            format_args!("if(style({condition}):{yes};else:{no})").to_lean_string()
        } else {
            if is_css_wide(yes) {
                return Err(DeclarationFailure::Skip(CSS_WIDE_BRANCH));
            }
            let name = helper(
                identifiers,
                ConditionRole::Fallback,
                &value[function.start..end],
            )?;
            // Writing through `fmt::Write` into a `LeanString` cannot fail.
            let _ = write!(helpers, "{name}:var({condition}) {yes};");
            format_args!("var({name},{no})").to_lean_string()
        };
        let delta = replacement.len().cast_signed() - (end - function.start).cast_signed();
        value.replace_range(function.start..end, &replacement);
        changes.push((function.start, delta));
    }
    Ok(format_args!("{helpers}{property}:{value}").to_lean_string())
}

/// Most `--if()` functions that one declaration can contain.
const MAX_FUNCTIONS: usize = 128;

/// Explains a declaration with more than [`MAX_FUNCTIONS`] `--if()` functions.
const TOO_MANY_FUNCTIONS: &str = "at most 128 --if expressions are supported per declaration";

/// Explains a value comparison that fallback lowering cannot represent.
const VALUE_COMPARISON: &str = "custom-property value comparisons require useCssIf: true";

/// Explains a CSS-wide keyword that a fallback helper would have to store.
const CSS_WIDE_BRANCH: &str = "CSS-wide branches stored in fallback helpers (including negated variants) require useCssIf: true";

/// A runtime condition that cannot be lowered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UnsupportedCondition {
    /// An `--if()` function is unclosed or does not have three valid arguments.
    Syntax,
    /// A declaration contains more `--if()` functions than lowering supports. The text
    /// states the limit.
    TooMany(&'static str),
    /// The fallback strategy cannot represent the condition. The text explains why.
    RequiresNative(&'static str),
}

impl UnsupportedCondition {
    /// Returns the explanation that a lint finding shows for this problem.
    pub(super) const fn detail(self) -> &'static str {
        match self {
            Self::Syntax => "expected --if(--condition, true-value, false-value)",
            Self::TooMany(detail) | Self::RequiresNative(detail) => detail,
        }
    }
}

/// Checks the runtime conditions that one utility candidate writes.
///
/// `values` contains the candidate's arbitrary values and properties, with underscores
/// decoded as spaces. `conditions` contains the canonical conditions of its conditional
/// variants. [`check_declarations`] checks the declarations that a utility definition
/// writes.
///
/// # Errors
///
/// Returns the first problem in source order. With `use_css_if` false, value
/// comparisons, negated variant conditions, and CSS-wide branches stored in fallback
/// helpers require native lowering.
pub(super) fn check_candidate<'a>(
    values: impl IntoIterator<Item = &'a str>,
    conditions: impl IntoIterator<Item = &'a str>,
    use_css_if: bool,
) -> Result<(), UnsupportedCondition> {
    for value in values {
        check_value(value, use_css_if)?;
    }
    if use_css_if {
        return Ok(());
    }
    for condition in conditions {
        let Some(condition) = RuntimeCondition::parse(condition) else {
            return Err(UnsupportedCondition::Syntax);
        };
        if condition.value.is_some() {
            return Err(UnsupportedCondition::RequiresNative(VALUE_COMPARISON));
        }
        if condition.negated {
            return Err(UnsupportedCondition::RequiresNative(
                "negated variant conditions require useCssIf: true",
            ));
        }
    }
    Ok(())
}

/// Checks that [`lower`] can lower every declaration of `css`, including declarations
/// in nested rules.
///
/// # Errors
///
/// Returns the first problem in source order, as [`check_candidate`] describes for
/// values.
pub(super) fn check_declarations(css: &str, use_css_if: bool) -> Result<(), UnsupportedCondition> {
    if !css.contains("--if(") {
        return Ok(());
    }
    declaration_values(css)
        .into_iter()
        .try_for_each(|(_, value)| check_value(&css[value], use_css_if))
}

/// Checks the `--if()` functions of one declaration value.
pub(super) fn check_value(value: &str, use_css_if: bool) -> Result<(), UnsupportedCondition> {
    let functions = find_css_functions(value, &["--if"], 0);
    if functions.len() > MAX_FUNCTIONS {
        return Err(UnsupportedCondition::TooMany(TOO_MANY_FUNCTIONS));
    }
    for function in functions {
        let Some(end) = function.end else {
            return Err(UnsupportedCondition::Syntax);
        };
        let Some((condition, yes, no)) =
            runtime_if_arguments(&value[function.arguments.start..end - 1])
        else {
            return Err(UnsupportedCondition::Syntax);
        };
        if use_css_if {
            continue;
        }
        if condition.value.is_some() {
            return Err(UnsupportedCondition::RequiresNative(VALUE_COMPARISON));
        }
        let stored = if condition.negated { no } else { yes };
        if is_css_wide(stored) {
            return Err(UnsupportedCondition::RequiresNative(CSS_WIDE_BRANCH));
        }
    }
    Ok(())
}

/// Returns whether `value` is a CSS-wide keyword that a fallback helper cannot store.
fn is_css_wide(value: &str) -> bool {
    matches!(
        value.to_lean_string().to_ascii_lowercase().as_str(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "revert-rule"
    )
}

/// One guarded declaration body and the gate declarations written into it.
pub(super) struct Guarded {
    /// The body, with the gates this pass wrote in front of the declarations they gate.
    pub(super) css: LeanStr,
    /// Names of those gates, which a later pass must not guard again.
    pub(super) gates: Vec<LeanStr>,
}

/// Guards every declaration, including nested rules, with a validated condition.
/// `condition` must satisfy `is_runtime_condition`. Emits unresolved `--if` values
/// with `revert-rule` as the false branch; callers must run [`lower`] and retain a
/// separate rule boundary. Existing guards combine as AND using local boolean gates.
/// Priority stays outside the expression; `known` names the gate declarations an
/// earlier pass wrote, which are left alone, and the ones this pass writes are
/// returned. Negated AND operands emit inversions that require native lowering.
///
/// # Errors
///
/// Returns the host generator's failure for a gate's name.
pub(super) fn guard(
    css: &str,
    condition: &str,
    identifiers: &IdentifierContext,
    known: &[LeanStr],
) -> Result<Guarded, CompileDiagnostic> {
    let mut gates = Vec::new();
    let css = map_declarations::<CompileDiagnostic>(css, |property, value| {
        if known.iter().any(|gate| gate == property.trim()) {
            return Ok(format_args!("{property}:{value}").to_lean_string());
        }
        let mut parser = Parser::new(value);
        let mut priority = None;
        loop {
            let start = parser.position().byte_index();
            match parser.next() {
                Ok(Token::Delim('!')) => {
                    if matches!(parser.next(), Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("important"))
                        && parser.is_exhausted()
                    {
                        priority = Some(start);
                        break;
                    }
                }
                Ok(token) if is_block(token) => {
                    consume(&mut parser);
                }
                Ok(_) => {}
                Err(_) => {
                    break;
                }
            }
        }
        let (value, priority) = priority.map_or((value, ""), |start| value.split_at(start));
        let mut gate = LeanString::new();
        let mut condition = condition.to_lean_string();
        let mut value = value;
        if let Some(arguments) = value
            .trim()
            .strip_prefix("--if(")
            .and_then(|v| v.strip_suffix(')'))
            && let Some((inner, yes, "revert-rule")) = runtime_if_arguments(arguments)
        {
            let inner = inner.to_lean_string();
            // A gate joins two conditions, and the join has to identify exactly one
            // pair: a condition value may itself contain `/`, so the separator is a
            // NUL, which no condition text carries.
            let name = helper(
                identifiers,
                ConditionRole::Gate,
                &format_args!("{condition}\0{inner}").to_lean_string(),
            )?;
            let left = gate_condition(&condition, &mut gate, identifiers, &mut gates)?;
            let right = gate_condition(&inner, &mut gate, identifiers, &mut gates)?;
            // Writing through `fmt::Write` into a `LeanString` cannot fail.
            let _ = write!(gate, "{name}:var({left}) var({right});");
            gates.push((name.clone()).into());
            condition = name;
            value = yes;
        }
        Ok(
            format_args!("{gate}{property}:--if({condition},{{{value}}},revert-rule){priority}")
                .to_lean_string(),
        )
    })?;
    Ok(Guarded {
        css: (css).into(),
        gates,
    })
}

/// Returns a custom-property name suitable for a fallback AND expression.
/// `condition` must satisfy `is_runtime_condition`. Positive presence tests return
/// their trimmed text without changing `declarations`; negations and comparisons
/// append an unresolved helper with empty/initial branches and return its name.
/// Callers must lower appended helpers in native mode before emitting them.
///
/// # Errors
///
/// Returns the host generator's failure for the helper's name.
fn gate_condition(
    condition: &str,
    declarations: &mut LeanString,
    identifiers: &IdentifierContext,
    gates: &mut Vec<LeanStr>,
) -> Result<LeanString, CompileDiagnostic> {
    let condition = condition.trim();
    let Some(parsed) = RuntimeCondition::parse(condition) else {
        return Ok(condition.to_lean_string());
    };
    if parsed.value.is_some() {
        let name = helper(identifiers, ConditionRole::GateValue, condition)?;
        // Writing through `fmt::Write` into a `LeanString` cannot fail.
        let _ = write!(declarations, "{name}:--if({condition},{{}},initial);");
        gates.push((name.clone()).into());
        return Ok(name);
    }
    if !parsed.negated {
        return Ok(condition.to_lean_string());
    }
    let property = parsed.name;
    let name = helper(identifiers, ConditionRole::GateNot, condition)?;
    // Writing through `fmt::Write` into a `LeanString` cannot fail.
    let _ = write!(
        declarations,
        "{name}:--if({},initial,{{}});",
        property.trim()
    );
    gates.push((name.clone()).into());
    Ok(name)
}

/// Identifies tokens whose nested contents can be consumed as one opaque value.
fn is_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}

/// Consumes the block opened by the parser's last token and returns its content-end
/// byte offset relative to the parser input. The last token must satisfy [`is_block`].
/// Nested parse errors are ignored; the parser advances past the block when available.
pub(super) fn consume(parser: &mut Parser<'_>) -> usize {
    let mut end = parser.position().byte_index();
    let _ = parser.parse_nested_block(|nested| {
        while nested.next_including_whitespace_and_comments().is_ok() {}
        end = nested.position().byte_index();
        Ok::<_, ParseError<()>>(())
    });
    end
}

/// Finds declaration values using token boundaries, with an explicit stack for rules.
/// Custom-property brace values are opaque, as are all function and URL contents.
/// Returns nonoverlapping property/value byte ranges into `css`, sorted by source
/// position. Semicolons are excluded; surrounding declaration whitespace is retained.
pub(super) fn declaration_values(css: &str) -> Vec<(Range<usize>, Range<usize>)> {
    let mut pending = Vec::new();
    pending.push(0..css.len());
    let mut values = Vec::new();
    while let Some(range) = pending.pop() {
        let fragment = &css[range.clone()];
        let mut parser = Parser::new(fragment);
        let mut start = 0;
        loop {
            let position = parser.position().byte_index();
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::Semicolon) => {
                    add_declaration(fragment, range.start, start..position, &mut values);
                    start = parser.position().byte_index();
                }
                Ok(Token::CurlyBracketBlock) => {
                    let mut header = Parser::new(&fragment[start..position]);
                    let custom = matches!(header.next(), Ok(Token::Ident(name)) if name.starts_with("--"))
                        && matches!(header.next(), Ok(Token::Colon));
                    let inner = parser.position().byte_index();
                    let end = consume(&mut parser);
                    if !custom {
                        pending.push(range.start + inner..range.start + end);
                        start = parser.position().byte_index();
                    }
                }
                Ok(token) if is_block(token) => {
                    consume(&mut parser);
                }
                Ok(_) => {}
                Err(_) => {
                    add_declaration(fragment, range.start, start..fragment.len(), &mut values);
                    break;
                }
            }
        }
    }
    values.sort_by_key(|(property, _)| property.start);
    values
}

/// Appends property/value ranges when `css[range]` starts with an identifier and colon.
/// `range` must lie on UTF-8 boundaries within `css`; `offset` translates its ranges
/// into the enclosing stylesheet. Unrecognized fragments leave `values` unchanged.
/// Panics if `range` is out of bounds or splits a UTF-8 character.
fn add_declaration(
    css: &str,
    offset: usize,
    range: Range<usize>,
    values: &mut Vec<(Range<usize>, Range<usize>)>,
) {
    let mut parser = Parser::new(&css[range.clone()]);
    if !matches!(parser.next(), Ok(Token::Ident(_))) {
        return;
    }
    if !matches!(parser.next(), Ok(Token::Colon)) {
        return;
    }
    let property_end = parser.position().byte_index() - 1;
    values.push((
        offset + range.start..offset + range.start + property_end,
        offset + range.start + parser.position().byte_index()..offset + range.end,
    ));
}

/// Replaces each declaration in source order with the result of `map(property, value)`.
/// The callback supplies the complete replacement, including its colon; surrounding
/// CSS and semicolons are retained. The first callback error aborts without returning
/// partial CSS, but any state changes made by earlier callback invocations remain.
fn map_declarations<E>(
    css: &str,
    mut map: impl FnMut(&str, &str) -> Result<LeanString, E>,
) -> Result<LeanString, E> {
    let mut output = LeanString::new();
    let mut cursor = 0;
    for (property, value) in declaration_values(css) {
        output.push_str(&css[cursor..property.start]);
        output.push_str(&map(&css[property], &css[value.clone()])?);
        cursor = value.end;
    }
    output.push_str(&css[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use lean_string::{LeanStr, ToLeanStr};

    use stillwind_source::ReplaceText;

    use super::*;
    use crate::identifiers::{DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierMode};

    /// One test's names, which the default generator always produces.
    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    /// Lowers with the compiler's own helper names.
    fn lower_with_default_names(
        css: &str,
        use_css_if: bool,
    ) -> Result<LeanString, CompileDiagnostic> {
        Ok(lower(css, use_css_if, &identifiers())?.css)
    }

    /// Guards with the compiler's own gate names.
    fn guard_with_default_names(css: &str, condition: &str) -> Result<Guarded, CompileDiagnostic> {
        guard(css, condition, &identifiers(), &[])
    }

    #[test]
    fn runtime_if_preserves_lexical_boundaries_and_priority() {
        let css = r".a:hover{content:'--if(--x,a,b)';background:url('--if(--x,a,b)');/*x*/ width : --if(--x,calc(1px + 2px),3px) ! IMPORTANT /*keep*/;}";
        let native = lower_with_default_names(css, true).unwrap();
        assert_eq!(
            native,
            css.replace_text(
                "--if(--x,calc(1px + 2px),3px)",
                "if(style(--x):calc(1px + 2px);else:3px)"
            )
        );
        assert_eq!(
            guard_with_default_names(
                "font-family:a,b;content:'!important';color:red ! IMPORTANT /*keep*/;",
                "--x"
            )
            .unwrap()
            .css,
            "font-family:--if(--x,{a,b},revert-rule);content:--if(--x,{'!important'},revert-rule);color:--if(--x,{red},revert-rule) ! IMPORTANT /*keep*/;"
        );
        let comma = lower_with_default_names(
            &guard_with_default_names("font-family:a,b;", "--x")
                .unwrap()
                .css,
            false,
        )
        .unwrap();
        assert!(comma.contains(":var(--x) a,b;"), "{comma}");
    }

    #[test]
    fn declarations_that_cannot_be_lowered_are_left_out() {
        let lowered = lower(
            "a{color:--if(--on,red);width:1px;top:--if(--on,inherit,0)}",
            false,
            &identifiers(),
        )
        .unwrap();
        assert_eq!(lowered.css, "a{;width:1px;}");
        assert_eq!(
            lowered.skipped,
            [
                SkippedConditional {
                    property: "color".into(),
                    detail: "expected --if(--condition, true-value, false-value)",
                },
                SkippedConditional {
                    property: "top".into(),
                    detail: CSS_WIDE_BRANCH,
                },
            ]
        );
    }

    #[test]
    fn runtime_if_nested_and_sibling_functions_keep_offsets() {
        let css = "x:--if(--a,--if(--b,1,2),--if(--c,3,4)) --if(--d,5,6);";
        assert_eq!(
            lower_with_default_names(css, true).unwrap(),
            "x:if(style(--a):if(style(--b):1;else:2);else:if(style(--c):3;else:4)) if(style(--d):5;else:6);"
        );
        let fallback = lower_with_default_names(css, false).unwrap();
        assert_eq!(fallback.matches(":var(--").count(), 5);
        assert!(!fallback.contains("--if("));
        assert_eq!(
            lower_with_default_names("x:--if(--a/*,*/,1,2);", true).unwrap(),
            "x:if(style(--a):1;else:2);"
        );
    }

    /// The name one guarded body allocated for its gate.
    fn gate_name(css: &str) -> LeanStr {
        let start = css.find("--sw-ifg-").expect("a gate name");
        let rest = &css[start..];
        rest[..rest.find(':').expect("the gate declaration")].to_lean_str()
    }

    #[test]
    fn a_gate_joins_two_conditions_without_ambiguity() {
        let identifiers = identifiers();
        let guard_gate = |condition: &str, inner: &str| {
            guard(
                &format_args!(".a{{width:--if({inner},{{1px}},revert-rule)}}").to_lean_str(),
                condition,
                &identifiers,
                &[],
            )
            .unwrap()
            .css
        };
        // `--n:1/--m:2` with `--k:3` and `--n:1` with `--m:2/--k:3` are different pairs whose
        // conditions join to the same characters, so each pair must still name its own gate.
        let first = guard_gate("--n:1/--m:2", "--k:3");
        let second = guard_gate("--n:1", "--m:2/--k:3");
        assert!(first.contains("--if(--n:1/--m:2,{},initial)"), "{first}");
        assert!(second.contains("--if(--m:2/--k:3,{},initial)"), "{second}");
        assert_ne!(gate_name(&first), gate_name(&second));
        assert_eq!(
            gate_name(&first),
            gate_name(&guard_gate("--n:1/--m:2", "--k:3"))
        );
    }

    #[test]
    fn runtime_if_guard_recurses_into_rules_but_not_custom_property_braces() {
        let css = ".a{--data:{x:y};@media print{&:hover{color:red;}}}";
        let guarded = guard_with_default_names(css, "--enabled").unwrap().css;
        assert!(
            guarded.contains("--data:--if(--enabled,{{x:y}},revert-rule)"),
            "{guarded}"
        );
        assert!(guarded.contains("color:--if(--enabled,{red},revert-rule)"));
        assert!(
            !lower_with_default_names(&guarded, false)
                .unwrap()
                .contains("--if(")
        );
    }
}

#[cfg(test)]
mod declaration_value_tests {
    use lean_string::{LeanStr, ToLeanStr};

    use super::declaration_values;

    fn slices(css: &str) -> Vec<(LeanStr, LeanStr)> {
        declaration_values(css)
            .into_iter()
            .map(|(property, value)| {
                (
                    css[property].trim().to_lean_str(),
                    css[value].trim().to_lean_str(),
                )
            })
            .collect()
    }

    /// Declarations are split at token-level semicolons, and empty or colon-less
    /// fragments contribute nothing.
    #[test]
    fn declaration_values_split_semicolon_separated_declarations() {
        assert_eq!(
            slices("a:1;b:2;"),
            [
                ("a".to_lean_str(), "1".to_lean_str()),
                ("b".to_lean_str(), "2".to_lean_str())
            ]
        );
        assert_eq!(slices("a:1;b:2"), slices("a:1;b:2;"));
        assert_eq!(slices(""), [] as [(LeanStr, LeanStr); 0]);
        assert_eq!(slices("no-colon;"), [] as [(LeanStr, LeanStr); 0]);
        // Surrounding whitespace is retained in both ranges.
        let css = "  a : 1 ; ";
        let values = declaration_values(css);
        assert_eq!(values.len(), 1);
        assert!(css[values[0].0.clone()].starts_with("  a "));
        assert!(css[values[0].1.clone()].starts_with(" 1 "));
    }

    /// Custom-property brace values, functions, and URLs are opaque.
    #[test]
    fn declaration_values_treat_braces_functions_and_urls_as_opaque() {
        assert_eq!(
            slices("--x:{a:1};y:2;"),
            [
                ("--x".to_lean_str(), "{a:1}".to_lean_str()),
                ("y".to_lean_str(), "2".to_lean_str())
            ]
        );
        assert_eq!(
            slices("x:url(a;b);y:calc(1;2);"),
            [
                ("x".to_lean_str(), "url(a;b)".to_lean_str()),
                ("y".to_lean_str(), "calc(1;2)".to_lean_str())
            ]
        );
        assert_eq!(
            slices("x:fn({a:1});y:2;"),
            [
                ("x".to_lean_str(), "fn({a:1})".to_lean_str()),
                ("y".to_lean_str(), "2".to_lean_str())
            ]
        );
    }

    /// Rule bodies are searched, and the results stay in source order without
    /// overlaps.
    #[test]
    fn declaration_values_descend_into_rule_bodies_in_source_order() {
        assert_eq!(
            slices("@media x{a:1;}b:2;"),
            [
                ("a".to_lean_str(), "1".to_lean_str()),
                ("b".to_lean_str(), "2".to_lean_str())
            ]
        );
        let css = "@media x{a:1;}b:2;";
        let values = declaration_values(css);
        let mut previous = 0;
        for (property, value) in &values {
            assert!(property.start >= previous);
            assert!(property.start <= value.start);
            previous = value.end;
        }
    }
}
