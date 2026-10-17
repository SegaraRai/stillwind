//! Names, syntax, and descriptions of Stillwind CSS directives and output slots.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

/// A directive accepted by the loader or authored CSS expansion.
pub(super) struct Directive {
    /// Decoded ASCII-lowercase at-rule name without `@`.
    pub(super) name: &'static str,
    /// Complete CSS example included in Markdown hover documentation.
    pub(super) syntax: &'static str,
    /// Prose explaining the directive's authored behavior.
    pub(super) description: &'static str,
}

/// At-rules offered by name completion and described by hover.
pub(super) const DIRECTIVES: &[Directive] = &[
    Directive {
        name: "apply",
        syntax: "@apply utilities;",
        description: "Inserts the declarations and nested variant rules of utility candidates into the enclosing rule. Unresolved candidates contribute no declarations.",
    },
    Directive {
        name: "variant",
        syntax: "@variant variant:sequence, alternative { ... }",
        description: "Inside a style rule, applies each comma-separated variant sequence to a copy of the block. Outside a style rule, a block containing @slot defines a variant.",
    },
    Directive {
        name: "theme",
        syntax: "@theme [inline | static] { --token: value; }",
        description: "Defines theme tokens. Inline tokens substitute their values into generated declarations. Static tokens are retained without a utility reference.",
    },
    Directive {
        name: "utility",
        syntax: "@utility name-* { ... }",
        description: "Defines a utility. A trailing * captures the candidate value for --value().",
    },
    Directive {
        name: "custom-variant",
        syntax: "@custom-variant name [scope(document)] (selector);",
        description: "Defines a variant with a selector shorthand or a block containing @slot. scope(document) lets the variant select a document-level element.",
    },
    Directive {
        name: "slot",
        syntax: "@slot;",
        description: "Marks where styled content is inserted into a variant definition.",
    },
    Directive {
        name: "stillwind-slot",
        syntax: "@stillwind-slot name;",
        description: "Places generated contributions in the authored stylesheet. theme and property-initials insert declarations; utilities, keyframes, and properties insert rules.",
    },
    Directive {
        name: "stillwind-css-template",
        syntax: "@stillwind-css-template name { ... }",
        description: "Defines the output template selected by style.css(). A template must contain a utilities slot and cannot contain design-system definitions.",
    },
    Directive {
        name: "stillwind-utility",
        syntax: "@stillwind-utility name-* [(tw-sort: property)] { ... }",
        description: "Registers a built-in utility with its authored cascade order.",
    },
    Directive {
        name: "stillwind-variant",
        syntax: "@stillwind-variant name [(descriptors)] { ... }",
        description: "Registers a built-in variant with optional value, modifier, scope, and ordering descriptors.",
    },
    Directive {
        name: "stillwind-property",
        syntax: "@stillwind-property --name { syntax: '*'; inherits: false; initial-value: 0; }",
        description: "Registers a custom property whose registration is emitted when generated CSS requires it.",
    },
    Directive {
        name: "stillwind-require-properties",
        syntax: "@stillwind-require-properties --name;",
        description: "Adds custom-property registrations to the requirements of a utility definition.",
    },
    Directive {
        name: "stillwind-arbitrary-property",
        syntax: "@stillwind-arbitrary-property pattern;",
        description: "Places arbitrary-property candidates in the authored utility order.",
    },
    Directive {
        name: "stillwind-animation-option",
        syntax: "@stillwind-animation-option name-* { ... }",
        description: "Defines declarations accepted as animation options.",
    },
    Directive {
        name: "stillwind-animation-option-property",
        syntax: "@stillwind-animation-option-property source-property: animation-property;",
        description: "Maps an ordinary utility property to the property emitted when it is used as an animation option.",
    },
    Directive {
        name: "stillwind-theme-namespace",
        syntax: "@stillwind-theme-namespace --namespace-* [type];",
        description: "Declares the value types accepted by a theme namespace.",
    },
    Directive {
        name: "stillwind-theme-order",
        syntax: "@stillwind-theme-order namespace ...;",
        description: "Sets the namespace order used to emit theme declarations.",
    },
    Directive {
        name: "stillwind-format",
        syntax: "@stillwind-format property format;",
        description: "Declares composition or value-typing metadata for matching properties.",
    },
    Directive {
        name: "stillwind-marker",
        syntax: "@stillwind-marker name;",
        description: "Registers a utility that emits a marker class.",
    },
    Directive {
        name: "stillwind-relationship",
        syntax: "@stillwind-relationship pattern;",
        description: "Registers a pattern that identifies relationship candidates.",
    },
    Directive {
        name: "stillwind-omit-token-prefix",
        syntax: "@stillwind-omit-token-prefix --prefix- condition;",
        description: "Prunes matching theme tokens when generated CSS contains the supplied condition.",
    },
    Directive {
        name: "stillwind-intrinsic",
        syntax: "@stillwind-intrinsic name;",
        description: "Selects a compiler intrinsic within a built-in definition.",
    },
];

/// Slot arguments that insert rules, paired with their hover descriptions.
pub(super) const RULE_SLOTS: &[(&str, &str)] = &[
    (
        "utilities",
        "Inserts generated utility rules. Place this slot outside style rules.",
    ),
    (
        "keyframes",
        "Inserts required keyframe rules. Place this slot outside style rules.",
    ),
    (
        "properties",
        "Inserts required @property registrations. Place this slot outside style rules.",
    ),
];

/// Slot arguments that insert declarations, paired with their hover descriptions.
pub(super) const DECLARATION_SLOTS: &[(&str, &str)] = &[
    (
        "theme",
        "Inserts required theme declarations into the enclosing style rule.",
    ),
    (
        "property-initials",
        "Inserts the initial values of required custom properties into the enclosing style rule.",
    ),
    (
        "property-initials inherited",
        "Inserts initial values only for registrations with inherits: true.",
    ),
    (
        "property-initials non-inherited",
        "Inserts initial values only for registrations with inherits: false.",
    ),
];

/// Returns the ASCII-lowercase at-rule name, decoding CSS escapes when present.
/// A partial token such as a lone `@` retains its written spelling.
pub(super) fn directive_name(text: &str) -> LeanStr {
    match Parser::new(text).next() {
        Ok(Token::AtKeyword(name)) => format_args!("@{}", name.to_ascii_lowercase()).to_lean_str(),
        _ => text.to_ascii_lowercase().into(),
    }
}
