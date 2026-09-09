use super::{BTreeSet, Ident, Parse, ParserOptions, PrinterOptions, StyleSheet, ToCss};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomUtility {
    pub name: String,
    pub body: String,
    /// Global registration order for built-in utilities (`@stillwind-utility`).
    pub builtin_order: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinArbitraryPropertyOrder {
    pub pattern: String,
    pub builtin_order: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomVariant {
    pub name: String,
    /// Whether the final `*` in `name` captures a value. Escaped `\*` in a
    /// definition header is decoded as a literal asterisk instead.
    pub wildcard: bool,
    pub selector: String,
    /// Full `@variant` body. Empty for the `@custom-variant` selector shorthand.
    pub body: String,
    /// Global registration order for built-in variants (`@stillwind-variant`).
    pub builtin_order: Option<usize>,
    /// Optional ordering behavior authored in the built-in definition header.
    pub sort: Option<BuiltinVariantSort>,
    /// Grammar and semantic kind of the wildcard capture.
    pub value_type: Option<BuiltinVariantValueType>,
    /// Grammar required when the capture contains a `/modifier` suffix.
    pub modifier_type: Option<BuiltinVariantModifierType>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantValueType {
    Variant,
    Aria,
    Data,
    Nth,
    SupportsCondition,
}

impl BuiltinVariantValueType {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "variant" => Some(Self::Variant),
            "aria" => Some(Self::Aria),
            "data" => Some(Self::Data),
            "nth" => Some(Self::Nth),
            "supports-condition" => Some(Self::SupportsCondition),
            _ => None,
        }
    }

    /// Validates CSS-defined captures whose grammar can be delegated to
    /// Lightning CSS. Compound variants and attribute shorthands are resolved
    /// structurally by the Tailwind registry instead.
    #[must_use]
    pub fn accepts_css(self, value: &str) -> bool {
        let stylesheet = match self {
            Self::Nth => format!(".x:nth-child({value}) {{ color: red; }}"),
            Self::SupportsCondition => {
                format!("@supports ({value}) {{ .x {{ color: red; }} }}")
            }
            Self::Variant | Self::Aria | Self::Data => return false,
        };
        StyleSheet::parse(&stylesheet, ParserOptions::default()).is_ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantModifierType {
    ClassIdentifier,
}

impl BuiltinVariantModifierType {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().strip_prefix('[')?.strip_suffix(']')? {
            "class-identifier" => Some(Self::ClassIdentifier),
            _ => None,
        }
    }

    /// Parses the logical class identifier and serializes it for insertion
    /// into a CSS selector, including any escapes required by CSS syntax.
    #[must_use]
    pub fn serialize(self, value: &str) -> Option<String> {
        match self {
            Self::ClassIdentifier => Ident::parse_string(value)
                .ok()?
                .to_css_string(PrinterOptions::default())
                .ok(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinVariantSortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuiltinVariantSort {
    pub group: Option<String>,
    pub key: Option<String>,
    pub direction: Option<BuiltinVariantSortDirection>,
    pub bare: bool,
}

/// One compiler-owned custom-property registration declared with
/// `@stillwind-property`. Ordinary authored `@property` rules are never added
/// to this registry and remain untouched in the output stylesheet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CustomPropertyActivation {
    Declared,
    #[default]
    Referenced,
    Either,
}

impl CustomPropertyActivation {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "declared" => Some(Self::Declared),
            "referenced" => Some(Self::Referenced),
            "either" => Some(Self::Either),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_active(self, declared: bool, referenced: bool) -> bool {
        match self {
            Self::Declared => declared,
            Self::Referenced => referenced,
            Self::Either => declared || referenced,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPropertyDefinition {
    pub name: String,
    pub body: String,
    pub activation: CustomPropertyActivation,
    pub dependencies: Vec<String>,
    pub groups: Vec<String>,
    pub activation_properties: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CustomPropertyUsage {
    pub declared: BTreeSet<String>,
    pub referenced: BTreeSet<String>,
    pub declaration_properties: BTreeSet<String>,
}
