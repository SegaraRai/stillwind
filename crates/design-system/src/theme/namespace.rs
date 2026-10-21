//! The theme namespaces that CSS declares, their value types and scales, and the token
//! names that a key selects in any namespace.
//!
//! `@stillwind-theme-namespace` declares each namespace that has a meaning to the
//! compiler: value types, a scale, or classification of `@theme` tokens. Rust code
//! knows only these kinds of meaning. It never names a namespace, so a stylesheet can
//! add, change, or remove a namespace without a Rust change. A custom property outside
//! every declared namespace never creates a value for a declared namespace. An authored
//! `--value(--size-*)` pattern can still read any namespace through
//! [`namespace_key_names`].

use std::collections::BTreeMap;

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{ValueType, extract_named_statements, split_top_level};
use stillwind_source::ReplaceText;

/// A theme namespace that a `@stillwind-theme-namespace` statement declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeNamespace {
    /// Namespace name without the leading `--` and the trailing `-*`, such as `spacing`
    /// for `--spacing-*`.
    pub name: LeanStr,
    /// Value types that arbitrary values and typed variables in this namespace must
    /// match, in authored order. When it is empty, every value is accepted.
    pub types: Vec<ValueType>,
    /// The numeric scale of this namespace, or `None` when the statement declares no
    /// scale.
    pub scale: Option<ThemeScale>,
}

impl ThemeNamespace {
    /// Returns the custom-property name of the scale token, such as `--spacing`. The
    /// public CSS function that multiplies the scale has the same name.
    #[must_use]
    pub fn scale_token(&self) -> LeanString {
        format_args!("--{}", self.name).to_lean_string()
    }
}

/// The numeric scale of a namespace.
///
/// The token whose name is the namespace name, such as `--spacing`, holds the scale.
/// A numeric key `n` resolves to a multiple of the scale, `--spacing(n)` lowers to the
/// same multiple, and the empty call `--spacing()` is a value domain of `--value()` and
/// `--modifier()`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeScale {
    /// Keys that resolve to a fixed value instead of a multiple of the scale, such as
    /// `px` to `1px`. They resolve only while the scale token is active.
    pub keys: BTreeMap<LeanStr, LeanStr>,
}

/// The theme namespaces that one stylesheet declares, and the narrower namespaces that
/// `--value()` patterns exclude.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeNamespaces {
    /// Declared namespaces by name.
    namespaces: BTreeMap<LeanStr, ThemeNamespace>,
    /// Names of narrower namespaces after `--{namespace}-`, by namespace name, such as
    /// `weight` for `font`.
    exclusions: BTreeMap<LeanStr, Vec<LeanStr>>,
}

impl ThemeNamespaces {
    /// Parses the `@stillwind-theme-namespace` and `@stillwind-theme-exclude`
    /// statements of `css`.
    ///
    /// `@stillwind-theme-namespace --name-* [type] ... scale(key: value, ...);`
    /// declares one namespace. Each `[type]` item adds an accepted value type, and
    /// unknown types and items are skipped. A `scale` item declares the namespace's
    /// scale, and its optional arguments add fixed keys. A later statement for the
    /// same namespace replaces an earlier one.
    ///
    /// `@stillwind-theme-exclude --name-* --name-narrower-* ...;` excludes the tokens
    /// of each narrower namespace from the first namespace. A narrower pattern that
    /// does not extend the first one is skipped.
    #[must_use]
    pub fn parse(css: &str) -> Self {
        let mut namespaces = Self::default();
        for (directive, definition) in ["stillwind-theme-namespace", "stillwind-theme-exclude"]
            .into_iter()
            .flat_map(|directive| {
                extract_named_statements(css, directive)
                    .into_iter()
                    .map(move |(_, definition)| (directive, definition))
            })
        {
            let mut items = top_level_items(&definition).into_iter();
            let Some(name) = items.next().and_then(pattern_name) else {
                continue;
            };
            match directive {
                "stillwind-theme-namespace" => {
                    let mut namespace = ThemeNamespace {
                        name: name.to_lean_str(),
                        types: Vec::new(),
                        scale: None,
                    };
                    for item in items {
                        if let Some(hint) = item
                            .strip_prefix('[')
                            .and_then(|item| item.strip_suffix(']'))
                        {
                            namespace.types.extend(ValueType::from_hint(hint));
                        } else if let Some(scale) = parse_scale(item) {
                            namespace.scale = Some(scale);
                        }
                    }
                    namespaces
                        .namespaces
                        .insert(namespace.name.clone(), namespace);
                }
                "stillwind-theme-exclude" => {
                    let narrower = items
                        .filter_map(pattern_name)
                        .filter_map(|narrower| narrower.strip_prefix(name)?.strip_prefix('-'))
                        .filter(|narrower| !narrower.is_empty())
                        .map(LeanStr::from);
                    namespaces
                        .exclusions
                        .entry(name.to_lean_str())
                        .or_default()
                        .extend(narrower);
                }
                _ => {}
            }
        }
        namespaces
    }

    /// Returns the declared namespace named `name`, such as `color`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ThemeNamespace> {
        self.namespaces.get(name)
    }

    /// Returns every declared namespace, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = &ThemeNamespace> {
        self.namespaces.values()
    }

    /// Returns every declared namespace that has a scale, ordered by name.
    pub fn scales(&self) -> impl Iterator<Item = &ThemeNamespace> {
        self.iter().filter(|namespace| namespace.scale.is_some())
    }

    /// Returns the namespace with a scale whose value domain is `domain`, such as
    /// `spacing` for `--spacing()`.
    #[must_use]
    pub fn scale_domain(&self, domain: &str) -> Option<&ThemeNamespace> {
        let name = domain.strip_prefix("--")?.strip_suffix("()")?;
        self.get(name).filter(|namespace| namespace.scale.is_some())
    }

    /// Classifies the custom property `name` by the longest declared namespace that it
    /// starts with.
    ///
    /// Returns the namespace and the token key after `--{namespace}-`, or the namespace
    /// and `None` when `name` is the namespace's bare name, such as `--spacing`.
    /// Returns `None` for a name outside every declared namespace.
    #[must_use]
    pub fn classify<'a>(&self, name: &'a str) -> Option<(&ThemeNamespace, Option<&'a str>)> {
        let rest = name.strip_prefix("--")?;
        if let Some(namespace) = self.get(rest) {
            return Some((namespace, None));
        }
        rest.match_indices('-').rev().find_map(|(index, _)| {
            let namespace = self.get(&rest[..index])?;
            Some((namespace, Some(&rest[index + 1..])))
        })
    }

    /// Returns the namespace with the longest name that the theme token `name` belongs
    /// to: `text-shadow` for `--text-shadow-sm` and `text` for `--text-sm`.
    ///
    /// Returns `None` for a name outside every namespace. It also returns `None` for a
    /// name with a second `--`, such as `--text-sm--line-height`, because that token
    /// holds a different property of `--text-sm`, and for a namespace reset such as
    /// `--color-*`.
    #[must_use]
    pub fn of_token(&self, name: &str) -> Option<&ThemeNamespace> {
        let rest = name.strip_prefix("--")?;
        if rest.contains("--") || rest.contains('*') {
            return None;
        }
        self.classify(name).map(|(namespace, _)| namespace)
    }

    /// Returns whether the token `name` is excluded from the namespace `namespace`
    /// because it belongs to a narrower namespace that `@stillwind-theme-exclude`
    /// names.
    ///
    /// For example, `--font-weight-bold` is excluded from `font` when `font` excludes
    /// `--font-weight-*`. `namespace` is written as in [`namespace_key_names`].
    #[must_use]
    pub fn excludes(&self, namespace: &str, name: &str) -> bool {
        let Some(narrower) = self.exclusions.get(namespace) else {
            return false;
        };
        let Some(key) = name
            .strip_prefix("--")
            .and_then(|name| name.strip_prefix(namespace))
            .and_then(|name| name.strip_prefix('-'))
        else {
            return false;
        };
        narrower.iter().any(|narrower| {
            key.strip_prefix(narrower.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
        })
    }
}

/// Reads a namespace pattern such as `--color-*` and returns its name, such as `color`.
fn pattern_name(pattern: &str) -> Option<&str> {
    let name = pattern.strip_prefix("--")?.strip_suffix("-*")?;
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    .then_some(name)
}

/// Reads a `scale` item: `scale` alone, or `scale(key: value, ...)` with fixed keys.
/// Returns `None` for another item or for an argument without `:`.
fn parse_scale(item: &str) -> Option<ThemeScale> {
    if item == "scale" {
        return Some(ThemeScale::default());
    }
    let arguments = item.strip_prefix("scale(")?.strip_suffix(')')?;
    let mut scale = ThemeScale::default();
    for argument in split_top_level(arguments, |token| matches!(token, Token::Comma))
        .into_iter()
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
    {
        let (key, value) = argument.split_once(':')?;
        scale
            .keys
            .insert(key.trim().to_lean_str(), value.trim().to_lean_str());
    }
    Some(scale)
}

/// Splits `text` at whitespace outside parentheses and brackets.
fn top_level_items(text: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0_usize;
    let mut start = None;
    for (index, character) in text.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if character.is_whitespace() && depth == 0 => {
                if let Some(begin) = start.take() {
                    items.push(&text[begin..index]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(index);
    }
    if let Some(begin) = start {
        items.push(&text[begin..]);
    }
    items
}

/// Returns the custom-property names that the candidate key `key` can select in the
/// namespace `namespace`, in lookup order, as Tailwind resolves
/// `--value(--namespace-*)`.
///
/// `namespace` is the namespace name without the leading `--` and the trailing `-`,
/// such as `size` or `size-lg`. The first name joins `namespace` and `key` unchanged.
/// When `key` contains `.`, the names with `_` in place of each `.` follow, so `1.5`
/// also selects `--size-1_5`. Each name is followed by its spelling with CSS escapes,
/// such as `--size-1\.5`, because `@theme` keeps token names as authored.
pub(super) fn namespace_key_names(namespace: &str, key: &str) -> Vec<LeanString> {
    let mut keys = vec![key.to_lean_string()];
    if key.contains('.') {
        keys.push(key.replace_char('.', "_"));
    }
    let mut names = Vec::new();
    for key in keys {
        names.push(format_args!("--{namespace}-{key}").to_lean_string());
        let escaped = escape_key(&key);
        if escaped != key {
            names.push(format_args!("--{namespace}-{escaped}").to_lean_string());
        }
    }
    names
}

/// Escapes each ASCII character of `key` that a CSS identifier cannot contain
/// unescaped.
fn escape_key(key: &str) -> LeanString {
    let mut escaped = LeanString::new();
    for character in key.chars() {
        if character.is_ascii()
            && !character.is_ascii_alphanumeric()
            && !matches!(character, '-' | '_')
        {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespace_value_types_and_scales() {
        let namespaces = ThemeNamespaces::parse(
            "@stillwind-utility duration-* { animation-duration: --value(--duration-*); }\
             @stillwind-theme-namespace --duration-* [time] [unknown];\
             @stillwind-theme-namespace --leading-* [number] [length];\
             @stillwind-theme-namespace --gap-* [length] scale(px: 1px, hair: 0.5px);\
             @stillwind-theme-namespace --step-* scale;\
             @stillwind-theme-namespace --bad scale;",
        );
        assert_eq!(namespaces.get("duration").unwrap().types, [ValueType::Time]);
        assert_eq!(
            namespaces.get("leading").unwrap().types,
            [ValueType::Number, ValueType::Length]
        );
        assert_eq!(namespaces.get("leading").unwrap().scale, None);
        let gap = namespaces.get("gap").unwrap();
        assert_eq!(
            gap.scale.as_ref().unwrap().keys,
            BTreeMap::from([("hair".into(), "0.5px".into()), ("px".into(), "1px".into())])
        );
        assert_eq!(gap.scale_token(), "--gap");
        assert_eq!(namespaces.get("step").unwrap().types, []);
        assert_eq!(
            namespaces
                .scales()
                .map(|namespace| namespace.name.as_str())
                .collect::<Vec<_>>(),
            ["gap", "step"]
        );
        assert_eq!(namespaces.scale_domain("--gap()").unwrap().name, "gap");
        assert_eq!(namespaces.scale_domain("--leading()"), None);
        assert_eq!(namespaces.get("bad"), None);
    }

    #[test]
    fn declarations_skip_comments_and_strings_and_keep_function_arguments() {
        let namespaces = ThemeNamespaces::parse(
            r#"/* @stillwind-theme-namespace --comment-* scale; */
               .example { content: "@stillwind-theme-namespace --quoted-* scale;"; }
               @stillwind-theme-namespace --step-* [length]
                   scale(hair: calc(1px + var(--hair, 1px)));
               @stillwind-theme-namespace --step-* [length]
                   scale(hair: var(--hair, 2px));
               /* @stillwind-theme-exclude --step-* --step-small-*; */"#,
        );
        assert!(namespaces.get("comment").is_none());
        assert!(namespaces.get("quoted").is_none());
        assert!(!namespaces.excludes("step", "--step-small-1"));
        assert_eq!(
            namespaces.get("step").unwrap().scale.as_ref().unwrap().keys["hair"],
            "var(--hair, 2px)"
        );
    }

    #[test]
    fn tokens_belong_to_the_longest_declared_namespace() {
        let namespaces = ThemeNamespaces::parse(
            "@stillwind-theme-namespace --text-*; @stillwind-theme-namespace --text-shadow-*;\
             @stillwind-theme-namespace --spacing-* scale;",
        );
        let name = |token| {
            namespaces
                .of_token(token)
                .map(|namespace| namespace.name.as_str())
        };
        assert_eq!(name("--text-sm"), Some("text"));
        assert_eq!(name("--text-shadow-sm"), Some("text-shadow"));
        assert_eq!(name("--spacing"), Some("spacing"));
        for token in ["--text-sm--line-height", "--my-len", "--text-*", "--*"] {
            assert_eq!(name(token), None, "{token}");
        }
        let classify = |token| {
            namespaces
                .classify(token)
                .map(|(namespace, key)| (namespace.name.as_str(), key))
        };
        assert_eq!(classify("--spacing"), Some(("spacing", None)));
        assert_eq!(
            classify("--text-shadow-sm"),
            Some(("text-shadow", Some("sm")))
        );
        assert_eq!(
            classify("--text-sm--line-height"),
            Some(("text", Some("sm--line-height")))
        );
        assert_eq!(classify("--textual"), None);
    }

    #[test]
    fn namespace_keys_try_underscores_for_dots_and_escaped_spellings() {
        assert_eq!(namespace_key_names("size", "4"), ["--size-4"]);
        assert_eq!(
            namespace_key_names("size-lg", "1.5"),
            ["--size-lg-1.5", r"--size-lg-1\.5", "--size-lg-1_5",]
        );
        assert_eq!(
            namespace_key_names("size", "1/2"),
            ["--size-1/2", r"--size-1\/2"]
        );
    }

    #[test]
    fn excluded_narrower_namespaces_follow_the_declared_patterns() {
        let namespaces = ThemeNamespaces::parse(
            "@stillwind-theme-exclude --font-* --font-weight-* --size-*;\
             @stillwind-theme-exclude --grid-row-* --grid-row-start-*;",
        );
        assert!(namespaces.excludes("font", "--font-weight-bold"));
        assert!(namespaces.excludes("font", "--font-weight"));
        assert!(namespaces.excludes("grid-row", "--grid-row-start-2"));
        assert!(!namespaces.excludes("font", "--font-weighty"));
        assert!(!namespaces.excludes("font", "--font-sans"));
        assert!(!namespaces.excludes("font", "--font-size-1"));
        assert!(!namespaces.excludes("size", "--size-weight-1"));
    }
}
