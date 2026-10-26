//! Variant definition parsing.
//!
//! `@custom-variant` and `@variant` are user-authored, so a definition is identified by
//! its body or selector alone. `@stillwind-variant` is compiler-owned and may add a
//! typed header - a value type, a modifier type, or sort metadata - which is parsed
//! here and used only for `@stillwind-variant`. Both forms may declare that a variant
//! selects a document-level target, as [`VariantScope`] describes: a
//! `@stillwind-variant` header with `scope: document`, or `@custom-variant` with
//! `scope(document)` after the name.
//!
//! One unescaped `*` captures the variant's argument between literal boundaries; an
//! escaped `\*` is a literal asterisk in the name. Capture and sort handling stay in
//! the evaluator; this module only records what the CSS declared and never invents
//! defaults for missing metadata.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};
use lightningcss::printer::PrinterOptions;
use lightningcss::rules::CssRule;
use lightningcss::rules::supports::SupportsCondition;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::{Parse, ToCss};
use lightningcss::values::ident::Ident;

use stillwind_css::{
    ValueType, extract_named_blocks, extract_named_statements, find_css_functions, split_top_level,
    strip_css_comments, whitespace_separated_ranges,
};

use crate::conditionals::RuntimeCondition;

/// Direction applied to a sort group's numeric values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StillwindVariantSortDirection {
    /// Smaller values sort first.
    Ascending,
    /// Larger values sort first.
    Descending,
}

/// Sort metadata authored in a `@stillwind-variant` header.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindVariantSort {
    /// Variants sharing a group are ordered by their group's first position; the name
    /// itself is the group when none is declared.
    pub group: Option<LeanStr>,
    /// Theme namespace pattern whose token value orders variants in the group.
    pub key: Option<LeanStr>,
    /// Which end of the key's ordering comes first.
    pub direction: Option<StillwindVariantSortDirection>,
    /// Whether the variant also matches a bare token name, such as `lg` for a
    /// breakpoint variant.
    pub bare: bool,
}

/// Whether the `&`-less selectors of a variant definition are relative to `&`.
///
/// Without a declared scope, a selector rule in a definition body is a nested style
/// rule, so `:hover { @slot; }` styles hovered descendants of `&`. A definition with
/// the document scope writes selectors for document-level elements, such as `:host` or
/// `:root`. Its selectors are kept as written: they set the target in document context,
/// and they stay document-wide where `&` already selects an element.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VariantScope {
    /// `&`-less selectors are relative to `&`, as in CSS Nesting.
    #[default]
    Nested,
    /// `&`-less selectors select document-level elements. Declared as `scope: document`
    /// in a `@stillwind-variant` header or as `scope(document)` after the name of a
    /// `@custom-variant`.
    Document,
}

impl VariantScope {
    fn parse(value: &str) -> Option<Self> {
        (value.trim() == "document").then_some(Self::Document)
    }
}

/// One variant definition, from user CSS or from the compiler-owned registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomVariant {
    /// Decoded name, retaining the capture `*` at `wildcard` when present.
    pub name: LeanStr,
    /// UTF-8 byte offset of the one unescaped capture `*` in the decoded name.
    /// `None` denotes an exact name; escaped asterisks remain literal characters.
    pub wildcard: Option<usize>,
    /// Selector shorthand from `@custom-variant name (selector)`.
    pub selector: LeanStr,
    /// Full `@variant` body. Empty for the `@custom-variant` selector shorthand.
    pub body: LeanStr,
    /// Byte offset of a `@stillwind-variant` directive in the parsed CSS. `None` for
    /// `@custom-variant` and `@variant`, whose order comes from
    /// [`crate::ThemeIndex::variant_positions`].
    pub position: Option<usize>,
    /// Optional ordering behavior authored in the `@stillwind-variant` header.
    pub sort: Option<StillwindVariantSort>,
    /// Grammar and semantic kind of the wildcard capture.
    pub value_type: Option<StillwindVariantValueType>,
    /// Grammar required when the capture contains a `/modifier` suffix.
    pub modifier_type: Option<StillwindVariantModifierType>,
    /// Whether `&`-less selectors of the body or shorthand are relative to `&`.
    pub scope: VariantScope,
}

impl CustomVariant {
    /// Returns captures that the wildcard of this definition can accept, written for
    /// the inside of `[...]` with `_` for spaces. A typed capture gives the samples of
    /// its [`StillwindVariantValueType`]. An untyped capture gives the samples of each
    /// `[hint]` domain of the `--value(...)` placeholders in the body, such as `1rem`
    /// for `--value([length])`. The result is empty when neither gives a sample.
    #[must_use]
    pub fn sample_captures(&self) -> Vec<LeanString> {
        if let Some(value_type) = self.value_type {
            return value_type
                .sample_captures()
                .iter()
                .map(ToLeanString::to_lean_string)
                .collect();
        }
        find_css_functions(&self.body, &["--value"], 0)
            .into_iter()
            .flat_map(|function| {
                split_top_level(&self.body[function.arguments], |token| {
                    matches!(token, Token::Comma)
                })
            })
            .filter_map(|domain| {
                ValueType::from_hint(domain.trim().strip_prefix('[')?.strip_suffix(']')?)?.samples()
            })
            .flatten()
            .map(|sample| sample.replace(' ', "_").to_lean_string())
            .collect()
    }

    /// Splits the decoded pattern around its one capture, preserving literal suffixes.
    /// Returns `None` for exact names or an inconsistent capture offset.
    pub fn wildcard_parts(&self) -> Option<(&str, &str)> {
        let index = self.wildcard?;
        if self.name.as_bytes().get(index) != Some(&b'*') {
            return None;
        }
        Some((
            self.name.get(..index)?,
            self.name.get(index.checked_add(1)?..)?,
        ))
    }

    /// Extracts a nonempty candidate capture matching both literal pattern boundaries.
    /// Returns `None` for an exact name, invalid offset, empty prefix, or mismatch.
    pub fn capture<'a>(&self, value: &'a str) -> Option<&'a str> {
        let (prefix, suffix) = self.wildcard_parts()?;
        if prefix.is_empty() {
            return None;
        }
        value
            .strip_prefix(prefix)?
            .strip_suffix(suffix)
            .filter(|capture| !capture.is_empty())
    }
}

/// Grammar and semantic kind of a variant's wildcard capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StillwindVariantValueType {
    /// The capture is itself a variant stack, applied inside the definition.
    Variant,
    /// `aria-*` style captures resolved through attribute selectors.
    Aria,
    /// `data-*` style captures resolved through attribute selectors.
    Data,
    /// An `nth-child()` argument validated by CSS itself.
    Nth,
    /// A `@supports` condition validated by CSS itself.
    SupportsCondition,
    /// A runtime custom-property presence test, negation, or value comparison.
    IfCondition,
}

impl StillwindVariantValueType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "variant" => Some(Self::Variant),
            "aria" => Some(Self::Aria),
            "data" => Some(Self::Data),
            "nth" => Some(Self::Nth),
            "supports-condition" => Some(Self::SupportsCondition),
            "if-condition" => Some(Self::IfCondition),
            _ => None,
        }
    }

    /// Returns captures that the type accepts, written for the inside of `[...]` with
    /// `_` for spaces. A variant capture gives a selector and an at-rule.
    #[must_use]
    pub fn sample_captures(self) -> &'static [&'static str] {
        match self {
            Self::Variant => &["&:hover", "@media_(width>1px)"],
            Self::Aria | Self::Data => &["pressed"],
            Self::Nth => &["1"],
            Self::SupportsCondition => &["display:grid"],
            Self::IfCondition => &["--flag"],
        }
    }

    /// Validates nth/support captures with Lightning CSS and `[if-condition]` captures
    /// with the shared runtime-condition parser.
    /// Compound variants and attribute shorthands are resolved structurally by the
    /// Tailwind registry instead, so those types report `false` here rather than
    /// accepting anything.
    ///
    /// A support capture is tested as the condition `(<value>)`. Parts parsed as
    /// `<general-enclosed>` are accepted only when negated or when they use
    /// `font-format()`, `font-tech()`, or `at-rule()`.
    #[must_use]
    pub fn accepts_css(self, value: &str) -> bool {
        // The selector and declaration complete the stylesheet that the CSS parser
        // requires to validate the capture. No theme or utility definitions are read.
        let stylesheet = match self {
            Self::IfCondition => return RuntimeCondition::parse(value).is_some(),
            Self::Nth => format_args!(".x:nth-child({value}) {{ color: red; }}").to_lean_string(),
            Self::SupportsCondition => {
                format_args!("@supports ({value}) {{ .x {{ color: red; }} }}").to_lean_string()
            }
            Self::Variant | Self::Aria | Self::Data => return false,
        };
        let Ok(stylesheet) = StyleSheet::parse(&stylesheet, ParserOptions::default()) else {
            return false;
        };
        match stylesheet.rules.0.as_slice() {
            [CssRule::Supports(rule)] => supports_condition_can_match(&rule.condition),
            _ => self == Self::Nth,
        }
    }
}

/// Whether every part of a `@supports` condition is a test that a browser can pass.
///
/// CSS evaluates text that is neither a declaration, a nested condition, nor a known
/// support function as `<general-enclosed>`, which is always false. `(--foo)` and
/// `var(--foo)` are such text. A part of a negated condition may also be
/// `<general-enclosed>`, because the browser treats it as false and the negation as
/// true. The known functions are `selector()`, `font-format()`, `font-tech()`, and
/// `at-rule()`.
fn supports_condition_can_match(condition: &SupportsCondition<'_>) -> bool {
    match condition {
        SupportsCondition::Not(_)
        | SupportsCondition::Declaration { .. }
        | SupportsCondition::Selector(_) => true,
        SupportsCondition::And(conditions) | SupportsCondition::Or(conditions) => {
            conditions.iter().all(supports_condition_can_match)
        }
        SupportsCondition::Unknown(text) => text.split_once('(').is_some_and(|(name, _)| {
            ["font-format", "font-tech", "at-rule"]
                .iter()
                .any(|function| name.eq_ignore_ascii_case(function))
        }),
    }
}

/// Grammar required when a capture carries a `/modifier` suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StillwindVariantModifierType {
    /// The modifier after `/` is a CSS class identifier, escaped for a selector.
    ClassIdentifier,
}

impl StillwindVariantModifierType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "class-identifier" => Some(Self::ClassIdentifier),
            _ => None,
        }
    }

    /// Parses the logical class identifier and serializes it for insertion into a CSS
    /// selector, including any escapes required by CSS syntax.
    #[must_use]
    pub fn serialize(self, value: &str) -> Option<LeanString> {
        match self {
            Self::ClassIdentifier => Ident::parse_string(value)
                .ok()?
                .to_css_string(PrinterOptions::default())
                .ok()
                .map(Into::into),
        }
    }
}

/// Parses compiler-owned `@stillwind-variant` definitions in source order.
///
/// `position` is the byte offset of each directive in `css`, as for
/// `@stillwind-utility`. A definition whose header is invalid is skipped.
#[must_use]
pub fn parse_stillwind_variant_definitions(css: &str) -> Vec<CustomVariant> {
    extract_named_blocks(css, "@stillwind-variant")
        .into_iter()
        .filter_map(|(offset, header, body)| {
            let header = parse_stillwind_variant_header(strip_css_comments(&header).trim())?;
            Some(CustomVariant {
                name: header.name,
                wildcard: header.wildcard,
                selector: (LeanString::new()).into(),
                body: strip_css_comments(&body).trim().into(),
                position: Some(offset),
                sort: header.sort,
                value_type: header.value_type,
                modifier_type: header.modifier_type,
                scope: header.scope,
            })
        })
        .collect()
}

/// The parsed header of a `@stillwind-variant` definition.
pub(crate) struct ParsedStillwindVariantHeader {
    /// Decoded definition name, retaining a capture asterisk at the recorded offset, if
    /// present.
    name: LeanStr,
    /// UTF-8 byte offset of the unescaped capture in the decoded name, if present.
    wildcard: Option<usize>,
    /// Optional ordering metadata parsed from the definition header.
    sort: Option<StillwindVariantSort>,
    /// Optional grammar of the wildcard capture.
    value_type: Option<StillwindVariantValueType>,
    /// Optional grammar of the slash modifier.
    modifier_type: Option<StillwindVariantModifierType>,
    /// Declared scope of `&`-less selectors in the body.
    scope: VariantScope,
}

/// Returns the decoded variant name and the optional UTF-8 byte offset of its capture `*`.
///
/// CSS escapes decode as literal name characters; one unescaped asterisk captures
/// between fixed prefix and suffix text. Empty names, multiple captures, whitespace,
/// comments, and tokens other than identifiers, leading `@`, and `*` return `None`.
pub(crate) fn parse_variant_name(value: &str) -> Option<(LeanStr, Option<usize>)> {
    let mut parser = Parser::new(value);
    let mut name = LeanString::new();
    let mut wildcard = None;
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Ident(literal) => {
                name.push_str(literal);
            }
            Token::AtKeyword(literal) if name.is_empty() => {
                name.push('@');
                name.push_str(literal);
            }
            Token::Delim('@') if name.is_empty() => {
                name.push('@');
            }
            Token::Delim('*') if wildcard.is_none() => {
                wildcard = Some(name.len());
                name.push('*');
            }
            _ => {
                return None;
            }
        }
    }
    (!name.is_empty()).then(|| (name.into(), wildcard))
}

/// Parses the optional `(key: value, ...)` descriptor list of a `@stillwind-variant`
/// header.
///
/// Unknown keys, unparsable values, and a missing parenthesis pair are all rejected, so
/// a mistyped `@stillwind-variant` registration fails visibly instead of silently
/// dropping its ordering or typing metadata.
fn parse_stillwind_variant_header(header: &str) -> Option<ParsedStillwindVariantHeader> {
    let name_end = whitespace_separated_ranges(header).first()?.end;
    let (name, wildcard) = parse_variant_name(&header[..name_end])?;
    let arguments = header[name_end..].trim();
    if arguments.is_empty() {
        return Some(ParsedStillwindVariantHeader {
            name,
            wildcard,
            sort: None,
            value_type: None,
            modifier_type: None,
            scope: VariantScope::Nested,
        });
    }
    let arguments = arguments.strip_prefix('(')?.strip_suffix(')')?;
    let mut sort = StillwindVariantSort::default();
    let mut has_sort = false;
    let mut value_type = None;
    let mut modifier_type = None;
    let mut scope = VariantScope::Nested;
    for descriptor in arguments.split(',') {
        let (key, value) = descriptor.trim().split_once(':')?;
        let value = value.trim();
        match key.trim() {
            "value" => {
                value_type = Some(StillwindVariantValueType::parse(value)?);
            }
            "modifier" => {
                modifier_type = Some(StillwindVariantModifierType::parse(value)?);
            }
            "scope" => {
                scope = VariantScope::parse(value)?;
            }
            "group" => {
                sort.group = Some(value.to_lean_str());
                has_sort = true;
            }
            "sort" => {
                sort.key = Some(value.to_lean_str());
                has_sort = true;
            }
            "direction" => {
                sort.direction = Some(match value {
                    "asc" => StillwindVariantSortDirection::Ascending,
                    "desc" => StillwindVariantSortDirection::Descending,
                    _ => return None,
                });
                has_sort = true;
            }
            "bare" => {
                sort.bare = value.parse().ok()?;
                has_sort = true;
            }
            _ => {
                return None;
            }
        }
    }
    Some(ParsedStillwindVariantHeader {
        name,
        wildcard,
        sort: has_sort.then_some(sort),
        value_type,
        modifier_type,
        scope,
    })
}

/// Parses `@custom-variant` and block `@variant` definitions without requiring a
/// complete theme stylesheet, so tools can build a registry once and share it at
/// runtime.
///
/// Only blocks whose body contains `@slot` are treated as `@variant` definitions; a
/// `@variant` block without a slot is ignored. Returns each definition with the byte
/// offset of its directive in `css`, in offset order. Later definitions override
/// earlier ones regardless of shorthand or block syntax, and repeated names are
/// retained.
#[must_use]
pub fn parse_variant_definitions(css: &str) -> Vec<(usize, CustomVariant)> {
    let mut definitions = ["@custom-variant", "@variant"]
        .into_iter()
        .flat_map(|directive| extract_named_statements(css, directive))
        .filter_map(|(offset, header)| {
            let split = header.find(char::is_whitespace)?;
            let (name, wildcard) = parse_variant_name(&header[..split])?;
            let (scope, selector) = split_custom_variant_scope(&header[split..])?;
            let selector = selector.strip_prefix('(')?.strip_suffix(')')?;
            Some((
                offset,
                name,
                wildcard,
                scope,
                selector.to_lean_string(),
                LeanString::new(),
            ))
        })
        .collect::<Vec<_>>();
    for directive in ["@custom-variant", "@variant"] {
        definitions.extend(extract_named_blocks(css, directive).into_iter().filter_map(
            |(offset, header, body)| {
                if directive == "@variant" && !body.contains("@slot") {
                    return None;
                }
                let header = header.trim();
                let split = header.find(char::is_whitespace).unwrap_or(header.len());
                let (name, wildcard) = parse_variant_name(&header[..split])?;
                let scope = if directive == "@custom-variant" {
                    split_custom_variant_scope(&header[split..])?.0
                } else {
                    VariantScope::Nested
                };
                Some((
                    offset,
                    name,
                    wildcard,
                    scope,
                    LeanString::new(),
                    body.into(),
                ))
            },
        ));
    }
    definitions.sort_by_key(|definition| definition.0);
    definitions
        .into_iter()
        .map(|(offset, name, wildcard, scope, selector, body)| {
            (
                offset,
                CustomVariant {
                    name,
                    wildcard,
                    selector: (selector).into(),
                    body: (body).into(),
                    position: None,
                    sort: None,
                    value_type: None,
                    modifier_type: None,
                    scope,
                },
            )
        })
        .collect()
}

/// Splits the optional `scope(document)` marker from the text after a `@custom-variant`
/// name, and returns the scope with the trimmed remaining text.
///
/// Returns `None` for a `scope(...)` marker with any other value, so a mistyped marker
/// drops the definition instead of registering a variant with nested selectors.
fn split_custom_variant_scope(text: &str) -> Option<(VariantScope, &str)> {
    let text = text.trim();
    let Some(marker) = text.strip_prefix("scope(") else {
        return Some((VariantScope::Nested, text));
    };
    let (value, rest) = marker.split_once(')')?;
    Some((VariantScope::parse(value)?, rest.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::variant_definitions;

    #[test]
    fn separates_stillwind_variants_from_tailwind_style_variants() {
        let css = "@utility user-* { color:red; }\
                       @stillwind-utility builtin-* { color:blue; }\
                       @variant user-state { &:hover { @slot; } }\
                       @stillwind-variant builtin-state { &:focus { @slot; } }";

        let user_variants = variant_definitions(css);
        let stillwind_variants = parse_stillwind_variant_definitions(css);
        assert_eq!(user_variants.len(), 1);
        assert_eq!(user_variants[0].name, "user-state");
        assert_eq!(user_variants[0].position, None);
        assert_eq!(stillwind_variants.len(), 1);
        assert_eq!(stillwind_variants[0].name, "builtin-state");
        assert_eq!(
            stillwind_variants[0].position,
            css.find("@stillwind-variant")
        );
    }

    #[test]
    fn serializes_class_identifier_modifiers_for_css_selectors() {
        let kind = StillwindVariantModifierType::ClassIdentifier;
        assert_eq!(kind.serialize("menu").as_deref(), Some("menu"));
        assert_eq!(
            kind.serialize(r"menu\:open").as_deref(),
            Some(r"menu\:open")
        );
        assert_eq!(kind.serialize("not an identifier"), None);
    }

    #[test]
    fn validates_nth_and_supports_conditions_through_lightning_css() {
        assert!(StillwindVariantValueType::Nth.accepts_css("3n + 1"));
        assert!(StillwindVariantValueType::Nth.accepts_css("odd"));
        assert!(!StillwindVariantValueType::Nth.accepts_css("not-an-nth"));
        assert!(StillwindVariantValueType::SupportsCondition.accepts_css("display: grid"));
        for (value, expected) in [
            ("--foo: var(--tw)", true),
            ("selector(a:hover)", true),
            ("font-format(woff2)", true),
            ("(display: grid) and (not (--foo))", true),
            ("(--foo)", false),
            ("var(--foo)", false),
            ("(display: grid) or (--foo)", false),
            ("unknown-function(x)", false),
        ] {
            assert_eq!(
                StillwindVariantValueType::SupportsCondition.accepts_css(value),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn parses_typed_variant_values_modifiers_and_literal_asterisks() {
        let variants = parse_stillwind_variant_definitions(
            r"@stillwind-variant \* { :is(& > *) { @slot; } }
               @stillwind-variant group-* (value: [variant], modifier: [class-identifier]) { @slot; }
               @stillwind-variant supports-* (value: [supports-condition]) { @slot; }",
        );

        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name, "*");
        assert!(variants[0].wildcard.is_none());
        assert!(variants[1].wildcard.is_some());
        assert_eq!(
            variants[1].value_type,
            Some(StillwindVariantValueType::Variant)
        );
        assert_eq!(
            variants[1].modifier_type,
            Some(StillwindVariantModifierType::ClassIdentifier)
        );
        assert_eq!(
            variants[2].value_type,
            Some(StillwindVariantValueType::SupportsCondition)
        );
    }

    #[test]
    fn parses_stillwind_variant_sort_options() {
        let variants = parse_stillwind_variant_definitions(
            "@stillwind-variant max-* (sort: --breakpoint-*, direction: desc) { @slot; }\
             @stillwind-variant min-* (group: responsive-min, sort: --breakpoint-*, direction: asc, bare: true) { @slot; }",
        );

        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants[0].sort.as_ref().unwrap().key.as_deref(),
            Some("--breakpoint-*")
        );
        assert_eq!(
            variants[0].sort.as_ref().unwrap().direction,
            Some(StillwindVariantSortDirection::Descending)
        );
        assert_eq!(
            variants[1].sort.as_ref().unwrap().group.as_deref(),
            Some("responsive-min")
        );
        assert!(variants[1].sort.as_ref().unwrap().bare);
    }

    #[test]
    fn parses_document_scope_markers() {
        let stillwind = parse_stillwind_variant_definitions(
            "@stillwind-variant host (scope: document) { :host { @slot; } }\
             @stillwind-variant hover { &:hover { @slot; } }",
        );
        assert_eq!(stillwind[0].scope, VariantScope::Document);
        assert_eq!(stillwind[1].scope, VariantScope::Nested);
        assert!(parse_stillwind_variant_header("host (scope: element)").is_none());

        let custom = variant_definitions(
            "@custom-variant host scope(document) (:host);\
             @custom-variant root scope(document) { :root { @slot; } }\
             @custom-variant hv (:hover);\
             @custom-variant bad scope(nested) (:host);",
        );
        let scopes = custom
            .iter()
            .map(|variant| {
                (
                    variant.name.as_str(),
                    variant.scope,
                    variant.selector.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            scopes,
            [
                ("host", VariantScope::Document, ":host"),
                ("root", VariantScope::Document, ""),
                ("hv", VariantScope::Nested, ":hover"),
            ]
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn functional_patterns_capture_between_decoded_literal_boundaries() {
        let variants = parse_stillwind_variant_definitions(
            r"
            @stillwind-variant when\(*\) (value: [if-condition]) { @stillwind-intrinsic builtin-if-variant; }
            @stillwind-variant literal\**\* { & { @slot; } }
        ",
        );
        assert_eq!(variants[0].name, "when(*)");
        assert_eq!(variants[0].capture("when(--c:v)"), Some("--c:v"));
        assert_eq!(variants[0].capture("when(--c)v"), None);
        assert_eq!(variants[0].capture("when()"), None);
        assert_eq!(
            variants[0].value_type,
            Some(StillwindVariantValueType::IfCondition)
        );
        assert_eq!(variants[1].capture("literal*value*"), Some("value"));
        assert!(parse_variant_name("two-*-*").is_none());
        assert!(parse_stillwind_variant_header(r"when\(*\) (value: [condition])").is_none());
    }

    #[test]
    fn variant_names_and_sort_headers_reject_invalid_metadata() {
        assert_eq!(
            parse_variant_name(r"literal\*"),
            Some(("literal*".into(), None))
        );
        assert_eq!(parse_variant_name("min-*"), Some(("min-*".into(), Some(4))));
        assert_eq!(parse_variant_name(""), None);
        assert!(parse_stillwind_variant_header("min-* (direction: sideways)").is_none());
        assert!(parse_stillwind_variant_header("min-* (unknown: x)").is_none());
        assert!(parse_stillwind_variant_header("min-* (bare: maybe)").is_none());
    }
}
