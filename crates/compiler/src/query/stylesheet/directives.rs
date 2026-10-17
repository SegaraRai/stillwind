//! Names, audience, syntax provenance, and usage positions of CSS directives and slots.
//!
//! These classifications control editor discovery. They do not validate placement,
//! guarantee Tailwind compatibility, or change the accepted grammar and lint policy.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

/// The audience that should discover a directive through name completion.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Purpose {
    /// Applications and themes, including extensions to utilities and animation
    /// options.
    Authoring,
    /// Utility libraries and metadata that governs registry resolution.
    Registry,
    /// Definitions that select a behavior implemented by the compiler.
    Compiler,
}

impl Purpose {
    /// Returns the audience label displayed in completion details and hover.
    fn label(self) -> &'static str {
        match self {
            Self::Authoring => "Authoring",
            Self::Registry => "Registry authoring",
            Self::Compiler => "Compiler intrinsic",
        }
    }
}

/// Syntax provenance, independent of audience and compatibility guarantees.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Family {
    /// Syntax derived from Tailwind, evaluated with Stillwind's semantics.
    TailwindDerived,
    /// Syntax introduced by Stillwind.
    Stillwind,
}

impl Family {
    /// Returns the provenance label displayed with the audience.
    fn label(self) -> &'static str {
        match self {
            Self::TailwindDerived => "Tailwind-derived syntax",
            Self::Stillwind => "Stillwind syntax",
        }
    }
}

/// Cursor locations used for discovery. These flags do not validate accepted CSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Positions(u16);

impl Positions {
    /// No supported at-rule position, including descriptor-only blocks.
    pub(super) const NONE: Self = Self(0);
    /// Rules in the stylesheet or a grouping rule outside a style rule.
    pub(super) const RULES: Self = Self(1);
    /// Declarations and nested rules of an element selected by a style rule.
    pub(super) const STYLE: Self = Self(2);
    /// The body of an authored or registry utility definition.
    pub(super) const UTILITY: Self = Self(4);
    /// The body of an authored or registry variant definition, including its wrappers.
    pub(super) const VARIANT: Self = Self(8);
    /// The body of a declaration definition for animation options.
    pub(super) const ANIMATION: Self = Self(16);
    /// Rules inside a named CSS output template.
    pub(super) const TEMPLATE: Self = Self(32);
    /// Input without an enclosing block that could be an incomplete CSS fragment.
    pub(super) const UNCERTAIN: Self = Self(64);
    /// The direct body of a registry variant, where an intrinsic can replace its body.
    pub(super) const INTRINSIC_VARIANT: Self = Self(128);

    /// Combines independent locations without changing either operand.
    pub(super) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Removes inherited locations when entering a block with a different role.
    pub(super) const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Returns whether the two sets share at least one location.
    pub(super) const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns whether a definition supplies a local reason to offer registry syntax.
    pub(super) const fn in_definition(self) -> bool {
        self.intersects(Self::UTILITY.union(Self::VARIANT).union(Self::ANIMATION))
    }
}

/// A directive accepted by the loader or authored CSS expansion.
pub(super) struct Directive {
    /// Decoded ASCII-lowercase at-rule name without `@`.
    pub(super) name: &'static str,
    /// Complete CSS example included in Markdown hover documentation.
    pub(super) syntax: &'static str,
    /// Prose explaining the directive's authored behavior.
    pub(super) description: &'static str,
    /// Audience that controls discovery independently of parser and lint policy.
    pub(super) purpose: Purpose,
    /// Provenance shown in details and used to rank otherwise equivalent suggestions.
    pub(super) family: Family,
    /// Locations where this directive is useful. Several flags may apply.
    pub(super) positions: Positions,
}

impl Directive {
    /// Returns whether name completion should offer this directive at `at`.
    /// `registry` enables library metadata; compiler intrinsics still require a
    /// matching definition.
    pub(super) fn visible(&self, at: Positions, registry: bool) -> bool {
        let fits = self.positions.intersects(at);
        let uncertain = at.intersects(Positions::UNCERTAIN);
        match self.purpose {
            Purpose::Authoring => fits || uncertain,
            Purpose::Registry => (registry && (fits || uncertain)) || (fits && at.in_definition()),
            Purpose::Compiler => fits,
        }
    }

    /// Returns a stable rank by location, audience, and provenance, before lexical
    /// ties.
    pub(super) fn priority(&self, at: Positions) -> u8 {
        u8::from(!self.positions.intersects(at)) * 6 + self.purpose as u8 * 2 + self.family as u8
    }

    /// Describes audience and provenance without implying deprecation or compatibility.
    pub(super) fn detail(&self) -> LeanStr {
        format_args!("{} · {}", self.purpose.label(), self.family.label()).to_lean_str()
    }
}

/// At-rules offered by name completion and described by hover.
pub(super) const DIRECTIVES: &[Directive] = &[
    Directive {
        name: "apply",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::STYLE,
        syntax: "@apply utilities;",
        description: "Inserts the declarations and nested variant rules of utility candidates into the enclosing rule. Unresolved candidates contribute no declarations.",
    },
    Directive {
        name: "variant",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::RULES.union(Positions::STYLE),
        syntax: "@variant variant:sequence, alternative { ... }",
        description: "Inside a style rule, applies each comma-separated variant sequence to a separate copy of the block. Outside style rules, a block containing @slot is accepted as a legacy variant definition. Use @custom-variant for new variant definitions.",
    },
    Directive {
        name: "theme",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::RULES,
        syntax: "@theme [inline | static] { --token: value; }",
        description: "Defines theme tokens. Inline tokens substitute their values into generated declarations. Static tokens are retained without a utility reference.",
    },
    Directive {
        name: "utility",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::RULES,
        syntax: "@utility name-* { ... }",
        description: "Defines a utility. A trailing * captures the candidate value for --value().",
    },
    Directive {
        name: "custom-variant",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::RULES,
        syntax: "@custom-variant name [scope(document)] (selector);",
        description: "Defines a variant with a selector shorthand or a block containing @slot. scope(document) lets the variant select a document-level element.",
    },
    Directive {
        name: "slot",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::VARIANT,
        syntax: "@slot;",
        description: "Marks where styled content is inserted into a variant definition.",
    },
    Directive {
        name: "stillwind-slot",
        purpose: Purpose::Authoring,
        family: Family::Stillwind,
        positions: Positions::RULES
            .union(Positions::STYLE)
            .union(Positions::TEMPLATE),
        syntax: "@stillwind-slot name;",
        description: "Places generated contributions in the authored stylesheet. theme and property-initials insert declarations; utilities, keyframes, and properties insert rules.",
    },
    Directive {
        name: "stillwind-css-template",
        purpose: Purpose::Authoring,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-css-template name { ... }",
        description: "Defines the output template selected by style.css(). A template must contain a utilities slot and cannot contain design-system definitions.",
    },
    Directive {
        name: "plugin",
        purpose: Purpose::Authoring,
        family: Family::TailwindDerived,
        positions: Positions::RULES,
        syntax: "@plugin '<name>' { option: value; }",
        description: "Loads a Stillwind plugin through the host. Options configure the plugin's CSS definitions and dynamic utility families.",
    },
    Directive {
        name: "stillwind-utility",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-utility name-* [(tw-sort: property)] { ... }",
        description: "Defines a registry utility with its authored cascade order.",
    },
    Directive {
        name: "stillwind-variant",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-variant name [(descriptors)] { ... }",
        description: "Defines a registry variant with optional value, modifier, scope, and ordering descriptors.",
    },
    Directive {
        name: "stillwind-property",
        purpose: Purpose::Authoring,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-property --name { syntax: '*'; inherits: false; initial-value: 0; }",
        description: "Registers a custom property whose registration is emitted when generated CSS requires it.",
    },
    Directive {
        name: "stillwind-require-properties",
        purpose: Purpose::Authoring,
        family: Family::Stillwind,
        positions: Positions::UTILITY,
        syntax: "@stillwind-require-properties --name;",
        description: "Adds custom-property registrations to the requirements of a utility definition.",
    },
    Directive {
        name: "stillwind-arbitrary-property",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-arbitrary-property pattern;",
        description: "Places arbitrary-property candidates in the authored utility order.",
    },
    Directive {
        name: "stillwind-animation-option",
        purpose: Purpose::Authoring,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-animation-option name-* { ... }",
        description: "Defines declarations accepted as animation options.",
    },
    Directive {
        name: "stillwind-animation-option-property",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-animation-option-property source-property: animation-property;",
        description: "Maps an ordinary utility property to the property emitted when it is used as an animation option.",
    },
    Directive {
        name: "stillwind-theme-namespace",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-theme-namespace --namespace-* [type];",
        description: "Declares the value types accepted by a theme namespace.",
    },
    Directive {
        name: "stillwind-theme-order",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-theme-order namespace ...;",
        description: "Sets the namespace order used to emit theme declarations.",
    },
    Directive {
        name: "stillwind-format",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-format property format;",
        description: "Declares composition or value-typing metadata for matching properties.",
    },
    Directive {
        name: "stillwind-marker",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-marker name;",
        description: "Registers a utility that emits a marker class.",
    },
    Directive {
        name: "stillwind-relationship",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-relationship pattern;",
        description: "Registers a pattern that identifies relationship candidates.",
    },
    Directive {
        name: "stillwind-omit-token-prefix",
        purpose: Purpose::Registry,
        family: Family::Stillwind,
        positions: Positions::RULES,
        syntax: "@stillwind-omit-token-prefix --prefix- condition;",
        description: "Prunes matching theme tokens when generated CSS contains the supplied condition.",
    },
    Directive {
        name: "stillwind-intrinsic",
        purpose: Purpose::Compiler,
        family: Family::Stillwind,
        positions: Positions::INTRINSIC_VARIANT,
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
