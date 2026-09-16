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
    ///
    /// A DOM class token and a class selector are one spelling, so one type covers both.
    UtilityClass, class_token;
    /// A ref marker class, written where a ref is attached.
    ///
    /// Generated code keys a ref's projection lookup by this token.
    RefMarker, object_key_class_token;
    /// A scope marker class, written where a scope is attached.
    ///
    /// Generated code keys a scope's projection lookup by this token.
    ScopeMarker, object_key_class_token;
    /// An `anchor-name` custom-ident, which names the element a ref anchors to.
    AnchorName, custom_ident;
    /// A `container-name` custom-ident, which `@container` queries select.
    ContainerName, custom_ident;
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName, custom_ident;
    /// A `view-transition-class` custom-ident, which is also the transition's identity.
    ViewTransitionClass, custom_ident;
    /// A custom property name, without its leading double hyphen.
    CustomPropertyName, custom_ident;
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

impl ScopeMarker {
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
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
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

impl CustomPropertyName {
    /// This name as a custom property, including its leading double hyphen.
    ///
    /// The two hyphens belong to the property grammar rather than to the name, so the
    /// compiler adds them here.
    pub fn property(&self) -> String {
        format!("--{}", escaped_identifier(self.as_str()))
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
/// A label reaches a generator as part of a declaration context. The built-in generator does
/// not embed it, but a generator that does writes it into a name that reaches the DOM, so it
/// is cleaned like a class token.
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

/// Replaces what a class token cannot carry, and keeps the result an own object key.
///
/// Generated code writes these tokens as the keys of a projection lookup, and the key is the
/// canonical token itself, so the token has to be an own entry of every object it is written
/// to. A name an object already carries would not become one, and `__proto__` would even
/// replace the prototype, so a colliding token gains one leading underscore here, where the
/// name is decided: every writer of the declaration then sees one spelling.
fn object_key_class_token(name: &str) -> Result<String, IdentifierNameError> {
    let canonical = class_token(name)?;
    Ok(if is_inherited_member(&canonical) {
        format!("_{canonical}")
    } else {
        canonical
    })
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

/// Whether every object already carries `name`, so it could not become an own entry.
fn is_inherited_member(name: &str) -> bool {
    matches!(
        name,
        "constructor"
            | "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "__proto__"
            | "__defineGetter__"
            | "__defineSetter__"
            | "__lookupGetter__"
            | "__lookupSetter__"
    )
}

/// Escapes one canonical name as a class selector.
fn class_selector(name: &str) -> String {
    format!(".{}", escaped_identifier(name))
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
