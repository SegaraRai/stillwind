//! Branded generated names: cleaning at construction, escaping at write.
//!
//! Each family is a newtype over its canonical spelling, so a name that exists has already been
//! cleaned, and an arbitrary string cannot be written as a generated name.
//!
//! A family's constructor cleans the name once. It replaces what the family's destinations cannot
//! carry, and it makes the name usable as a JavaScript object key where the family is written as
//! one. A built-in name and a host-provided name are cleaned the same way: every built-in name
//! already satisfies these rules, so the step never changes a built-in name.
//!
//! A render method escapes for one destination, and a family declares only the render methods its
//! write sites use. A class token is written to a class attribute and to class selectors as the
//! same spelling, so one type covers both writes and the token itself is never escaped: the DOM
//! and the selector share the canonical spelling. A custom-ident family is written to value
//! positions, where the grammar can escape every character the spelling allows.
//!
//! Escaping is not part of the name. Comparisons, reports, and DOM tokens use the canonical
//! spelling, so a name never changes meaning by being written somewhere.

use std::fmt;

use cssparser::serialize_identifier;

use crate::generated::js_string;

/// A name a family has not produced yet.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdentifierNameError {
    /// The name holds no characters the family can carry.
    #[error("a generated name cannot be empty")]
    Empty,
}

/// A canonical generated name of one family.
///
/// The identity plumbing builds and compares names through this trait. Cleaning a name into a
/// family and writing it to a destination are each family's own business, so the inherent
/// constructor and the render methods are declared per family.
pub trait Token: Sized {
    /// Cleans `name` into this family's canonical spelling.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
    fn new(name: &str) -> Result<Self, IdentifierNameError>;

    /// The canonical spelling, which is not escaped.
    fn as_str(&self) -> &str;
}

/// Declares one family's brand type, its constructor, and the plumbing [`Token`] impl.
///
/// Render methods stay out of the macro. Which destinations a family reaches, and how each one
/// escapes, is what a reader of that family needs to see next to it.
macro_rules! brand {
    ($(
        $(#[$meta:meta])*
        $name:ident, $clean:expr;
    )*) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(String);

            impl $name {
                /// Cleans `name` into this family's canonical spelling.
                ///
                /// # Errors
                ///
                /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
                pub fn new(name: &str) -> Result<Self, IdentifierNameError> {
                    ($clean)(name).map(Self)
                }

                /// The canonical spelling, which is not escaped.
                pub fn as_str(&self) -> &str {
                    &self.0
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str(&self.0)
                }
            }

            impl Token for $name {
                fn new(name: &str) -> Result<Self, IdentifierNameError> {
                    Self::new(name)
                }

                fn as_str(&self) -> &str {
                    Self::as_str(self)
                }
            }
        )*
    };
}

brand! {
    /// A generated utility class, written to a class attribute and to class selectors.
    UtilityClass, class_token;
    /// A ref marker class, written where a ref is attached.
    RefMarker, class_token;
    /// An `anchor-name` value, which names the element a ref anchors to.
    AnchorName, custom_ident;
    /// A `container-name` custom-ident, which `@container` queries select.
    ContainerName, custom_ident;
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName, custom_ident;
    /// A `view-transition-class` custom-ident, which is also the transition's identity.
    ViewTransitionClass, custom_ident;
    /// A conditional helper or support-query name, without its leading double hyphen.
    ConditionHelperName, custom_ident;
    /// The custom property name one guarded view-transition use writes its class token
    /// into, without its leading double hyphen.
    ViewTransitionSlotName, custom_ident;
    /// A `@layer` name.
    LayerName, layer_name;
}

impl UtilityClass {
    /// This name as a class selector, including its leading dot.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }
}

impl RefMarker {
    /// This name as a class selector, including its leading dot.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl AnchorName {
    /// This name as the value `anchor-name`, `position-anchor`, and `anchor-scope` take.
    ///
    /// Those properties accept a `<dashed-ident>`, so the leading double hyphen belongs
    /// to the value rather than to the name.
    pub fn dashed_ident(&self) -> String {
        dashed_ident(self.as_str())
    }
}

impl ContainerName {
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }
}

impl KeyframesName {
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl ViewTransitionClass {
    /// This token as a `view-transition-class` value, in a declaration or a property value.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }

    /// This token as the class selector a view-transition pseudo-element selects it by.
    ///
    /// `::view-transition-group()`, `::view-transition-old()`, and `::view-transition-new()`
    /// select participants by their class, so those rules write the selector spelling.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }

    /// This token as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl ConditionHelperName {
    /// This name as a custom property, including its leading double hyphen.
    ///
    /// The two hyphens belong to the property grammar rather than to the name, so the
    /// compiler adds them here.
    pub fn property(&self) -> String {
        dashed_ident(self.as_str())
    }
}

impl ViewTransitionSlotName {
    /// This name as a custom property, including its leading double hyphen.
    ///
    /// The two hyphens belong to the property grammar rather than to the name, so the
    /// compiler adds them here.
    pub fn property(&self) -> String {
        dashed_ident(self.as_str())
    }
}

impl LayerName {
    /// This name as the `@layer` prelude, written after the at-keyword.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }
}

/// An authored label, cleaned for use inside a generated name.
///
/// A label reaches a generator as part of a declaration context. The built-in generator
/// spells it into every name the declaration generates, and those names reach the DOM, so
/// it is cleaned like a class token.
///
/// An empty label is no label: a declaration whose authored label cleans to nothing passes
/// `None` in its context and is named without one, rather than failing.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Label(String);

impl Label {
    /// Cleans `name` into a label.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
    pub fn new(name: &str) -> Result<Self, IdentifierNameError> {
        class_token(name).map(Self)
    }

    /// The canonical spelling, which is not escaped.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Token for Label {
    fn new(name: &str) -> Result<Self, IdentifierNameError> {
        Self::new(name)
    }

    fn as_str(&self) -> &str {
        Self::as_str(self)
    }
}

/// Replaces what a class token cannot carry.
///
/// A class attribute is whitespace-separated, so a token cannot hold whitespace, and NUL has
/// no CSS spelling at all. Both become `_`; every other character is kept, because a selector
/// and a JavaScript string can escape it.
fn class_token(name: &str) -> Result<String, IdentifierNameError> {
    let canonical: String = name
        .chars()
        .map(|character| match character {
            '\0' => '_',
            character if character.is_whitespace() => '_',
            character => character,
        })
        .collect();
    non_empty(canonical)
}

/// Keeps a custom-ident as written.
///
/// A custom-ident value can escape every character it can hold, so nothing is replaced. Colon
/// and slash stay in the spelling for the same reason: a selector escapes them.
fn custom_ident(name: &str) -> Result<String, IdentifierNameError> {
    non_empty(name.to_owned())
}

/// Replaces what a `@layer` name cannot carry.
///
/// A dot would name a nested layer instead of a layer of its own, so it becomes `_`.
fn layer_name(name: &str) -> Result<String, IdentifierNameError> {
    non_empty(name.replace('.', "_"))
}

/// Rejects a name that holds nothing.
fn non_empty(name: String) -> Result<String, IdentifierNameError> {
    if name.is_empty() {
        Err(IdentifierNameError::Empty)
    } else {
        Ok(name)
    }
}

/// Escapes one canonical name as a class selector.
fn class_selector(name: &str) -> String {
    format!(".{}", escaped_identifier(name))
}

/// Escapes one canonical name as a `--`-prefixed identifier.
///
/// A custom property name and a `<dashed-ident>` value share this spelling: the two
/// hyphens belong to the property or value grammar, not to the name.
fn dashed_ident(name: &str) -> String {
    escaped_identifier(&format!("--{name}"))
}

/// Escapes one canonical name as a CSS identifier.
pub(crate) fn escaped_identifier(name: &str) -> String {
    let mut escaped = String::new();
    // Writing through `fmt::Write` into a `String` cannot fail.
    let _ = serialize_identifier(name, &mut escaped);
    escaped
}

/// Composes the `view-transition-name` one token and key are paired under.
///
/// A view-transition-name is a pairing key rather than an allocated name: it is derived
/// from the participant's class token and the key as written, escaped for a value
/// position, and two different transitions may compose the same value on purpose.
pub(crate) fn view_transition_name(token: &str, key: &str) -> String {
    escaped_identifier(&format!("{token}-{key}"))
}

#[cfg(test)]
mod tests {
    use super::view_transition_name;

    #[test]
    fn dashed_names_can_start_with_digits_without_escape_overhead() {
        assert_eq!(
            super::ConditionHelperName::new("123ab").unwrap().property(),
            "--123ab"
        );
        assert_eq!(
            super::ConditionHelperName::new("a:b").unwrap().property(),
            "--a\\:b"
        );
    }

    /// The runtime composes the dynamic half of these names, so both sides read one table.
    #[test]
    fn composes_names_matching_the_shared_escape_vectors() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/view-transitions/name-escapes.json"
        );
        let text = std::fs::read_to_string(path).expect("the shared escape vectors");
        let vectors: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let token = vectors["token"].as_str().expect("a token");
        let entries = vectors["vectors"].as_array().expect("vectors");
        assert_ne!(entries, &[] as &[serde_json::Value; 0]);
        for entry in entries {
            let key = entry["key"].as_str().expect("a key");
            let expected = entry["name"].as_str().expect("a name");
            assert_eq!(view_transition_name(token, key), expected, "key {key:?}");
        }
    }
}
