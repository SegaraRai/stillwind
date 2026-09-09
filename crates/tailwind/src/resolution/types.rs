use super::Candidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    /// Source order of the matched `@stillwind-utility`.
    pub builtin_order: Option<usize>,
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads.
    /// CSS fallbacks and selector-bearing branches must never be discarded
    /// merely because they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
}

/// A conditional declaration authored inside a CSS `@utility` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedUtilityRule {
    pub at_rule: NestedUtilityAtRule,
    pub condition: String,
    pub declaration: String,
    /// Whether the conditional was authored inside the utility's selector
    /// block (or as a CSS `@supports` declaration, which is nested by CSS).
    /// This preserves placement from the CSS source instead of classifying
    /// declarations by Tailwind property names.
    pub inside: bool,
}

/// The CSS conditional wrapper associated with a nested utility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedUtilityAtRule {
    Media,
    Supports,
}

pub(super) type ParsedNestedUtilityRule = (NestedUtilityAtRule, String, String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UtilityResolution {
    pub(super) declaration: String,
    pub(super) selector: Option<String>,
    pub(super) nested: Vec<NestedUtilityRule>,
    pub(super) additional: Vec<ResolvedUtilityTarget>,
    pub(super) builtin_order: Option<usize>,
}

impl UtilityResolution {
    pub(super) fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
            builtin_order: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ResolvedCssDeclarations {
    pub(super) base: String,
    pub(super) compat_modern: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompatColorMode {
    Legacy,
    Modern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    Compound(Vec<VariantApplication>),
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
    },
    SelectorMedia {
        selector: String,
        condition: String,
    },
    SelectorMediaRule {
        selector: String,
        declarations: Vec<VariantDeclaration>,
        condition: String,
    },
    Media(String),
    Container(String),
    Supports(String),
    StartingStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    pub property: String,
    pub value: String,
}
