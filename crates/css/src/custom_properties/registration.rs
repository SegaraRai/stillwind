//! Compiler-owned custom-property registrations and authored `@property` names.
//!
//! `@stillwind-property --name { ... }` declares a registration the compiler may emit.
//! Its descriptors are split in two: activation metadata (`activation`, `depends-on`,
//! `group`, `activate-on`) is consumed here, and everything else is kept verbatim as
//! the standard `@property` body that will be emitted. The two must not be mixed,
//! because the emitted block has to be valid `@property` CSS.
//!
//! Authored `@property` rules are a separate, read-only concern:
//! `extract_authored_property_names` reports their names so that compiler-owned
//! registrations never shadow them, and `extract_authored_property_syntaxes` reports
//! their `syntax` descriptors for lint rules that check variable types.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr};

use stillwind_source::JoinText;

use crate::{extract_named_blocks, parse_declaration_list};

/// One parsed `@stillwind-property` registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPropertyDefinition {
    /// Property name including the leading `--`.
    pub name: LeanStr,
    /// Standard `@property` descriptors, one per line, with the activation metadata
    /// removed. Emitted verbatim inside the generated block.
    pub body: LeanStr,
    /// When the compiler should emit this registration.
    pub activation: CustomPropertyActivation,
    /// Custom properties this registration needs; each dependency is itself a candidate
    /// for emission.
    pub dependencies: Vec<LeanStr>,
    /// Free-form group names, used to activate registrations together.
    pub groups: Vec<LeanStr>,
    /// Ordinary property names that activate this registration when the output declares
    /// a value for them.
    pub activation_properties: Vec<LeanStr>,
}

/// One compiler-owned custom-property registration declared with
/// `@stillwind-property`. Ordinary authored `@property` rules are never added to this
/// registry and remain untouched in the output stylesheet.
///
/// The default is `Referenced`: a registration is emitted only when something reads the
/// property, which keeps unused descriptors out of the output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CustomPropertyActivation {
    /// Emit only when required by another registration's dependency or group.
    Dependency,
    /// Emit when the property itself is declared with a value.
    Declared,
    /// Emit when the property is read through `var()`.
    #[default]
    Referenced,
    /// Emit when either condition holds.
    Either,
}

impl CustomPropertyActivation {
    /// Parses the `activation:` descriptor value; unknown values are ignored by the
    /// caller, which keeps the default in place.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "dependency" => Some(Self::Dependency),
            "declared" => Some(Self::Declared),
            "referenced" => Some(Self::Referenced),
            "either" => Some(Self::Either),
            _ => None,
        }
    }

    /// Whether this policy is satisfied by the observed usage of one property.
    ///
    /// Both flags come from an emitted stylesheet, so activation is decided after
    /// generation rather than predicted from configuration.
    #[must_use]
    pub fn is_active(self, declared: bool, referenced: bool) -> bool {
        match self {
            Self::Dependency => false,
            Self::Declared => declared,
            Self::Referenced => referenced,
            Self::Either => declared || referenced,
        }
    }
}

/// Parses compiler-owned custom-property registrations (`@stillwind-property`) in
/// source order.
///
/// A registration inside a comment is not parsed. Blocks without a `--` name are
/// skipped rather than reported, and so are body statements that are not `name: value`
/// declarations.
#[must_use]
pub fn parse_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_named_blocks(css, "@stillwind-property")
        .into_iter()
        .filter_map(|(_, header, body)| {
            let name = property_name(&header)?;
            let mut activation = CustomPropertyActivation::default();
            let mut dependencies = Vec::new();
            let mut groups = Vec::new();
            let mut activation_properties = Vec::new();
            let mut standard_descriptors = Vec::new();
            for descriptor in parse_declaration_list(&body).into_iter().flatten() {
                let value = descriptor.value.as_str();
                match descriptor.name.as_str() {
                    "activation" => {
                        if let Some(parsed) = CustomPropertyActivation::parse(value) {
                            activation = parsed;
                        }
                    }
                    "depends-on" => {
                        dependencies.extend(value.split(' ').map(LeanStr::from));
                    }
                    "group" => {
                        groups.extend(value.split(' ').map(LeanStr::from));
                    }
                    "activate-on" => {
                        activation_properties.extend(value.split(' ').map(LeanStr::from));
                    }
                    name => {
                        standard_descriptors.push(format_args!("{name}: {value};").to_lean_str());
                    }
                }
            }
            Some(CustomPropertyDefinition {
                name,
                body: (standard_descriptors.join_text("\n")).into(),
                activation,
                dependencies,
                groups,
                activation_properties,
            })
        })
        .collect()
}

/// Collects the names of authored `@property` rules.
///
/// Names are gathered so the compiler can avoid emitting a registration for a property
/// the author already registers; nothing is removed from the output.
pub fn extract_authored_property_names(css: &str) -> BTreeSet<LeanStr> {
    extract_named_blocks(css, "@property")
        .into_iter()
        .filter_map(|(_, header, _)| property_name(&header))
        .collect()
}

/// Collects the `syntax` descriptor of each authored `@property` rule, keyed by
/// property name.
///
/// The value is the descriptor's string content without quotes, such as `<length>`. A
/// rule without a quoted `syntax` descriptor is absent. A later rule for the same name
/// replaces an earlier one.
pub fn extract_authored_property_syntaxes(css: &str) -> BTreeMap<LeanStr, LeanStr> {
    extract_named_blocks(css, "@property")
        .into_iter()
        .filter_map(|(_, header, body)| {
            let name = property_name(&header)?;
            let syntax = parse_declaration_list(&body)
                .into_iter()
                .flatten()
                .find(|descriptor| descriptor.name.eq_ignore_ascii_case("syntax"))?;
            let mut parser = Parser::new(&syntax.value);
            let syntax = parser.expect_string().ok()?.trim().to_lean_str();
            parser.is_exhausted().then_some((name, syntax))
        })
        .collect()
}

/// Returns the authored spelling of the custom-property name that starts an at-rule
/// prelude, or `None` when the first token after whitespace and comments is not an
/// identifier that starts with `--`.
fn property_name(prelude: &str) -> Option<LeanStr> {
    let mut parser = Parser::new(prelude);
    let name = loop {
        let start = parser.position();
        match parser.next_including_whitespace_and_comments().ok()? {
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::Ident(_) => break parser.slice_from(start),
            _ => return None,
        }
    };
    (name.len() > 2 && name.starts_with("--")).then(|| name.to_lean_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_property_syntaxes_are_read_from_quoted_descriptors() {
        let syntaxes = extract_authored_property_syntaxes(
            "@property --len { syntax: \"<length>\"; inherits: false; }
             @property --any { syntax: '*'; }
             @property --none { inherits: true; }
             /* @property --comment { syntax: \"<color>\"; } */",
        );
        assert_eq!(
            syntaxes,
            BTreeMap::from([
                ("--any".into(), "*".into()),
                ("--len".into(), "<length>".into()),
            ])
        );
    }

    #[test]
    fn parses_property_activation_and_dependency_metadata() {
        let definitions = parse_property_definitions(
            r#"
                @stillwind-property --example {
                  syntax: "*";
                  inherits: false;
                  activation: referenced;
                  depends-on: --dependency-a --dependency-b;
                  group: example-group shared-group;
                  activate-on: text-shadow box-shadow;
                }
            "#,
        );

        assert_eq!(definitions.len(), 1);
        assert_eq!(
            definitions[0].activation,
            CustomPropertyActivation::Referenced
        );
        assert_eq!(
            definitions[0].dependencies,
            ["--dependency-a", "--dependency-b"]
        );
        assert_eq!(definitions[0].groups, ["example-group", "shared-group"]);
        assert_eq!(
            definitions[0].activation_properties,
            ["text-shadow", "box-shadow"]
        );
        assert!(!definitions[0].body.contains("activation"));
        assert!(!definitions[0].body.contains("depends-on"));
        assert!(!definitions[0].body.contains("group"));
        assert!(!definitions[0].body.contains("activate-on"));
    }

    #[test]
    fn registrations_ignore_comments_in_names_and_descriptors() {
        let definitions = parse_property_definitions(
            r#"@stillwind-property /* name */ --example {
                  /* group: ignored; */
                  syntax: "*"; /* activation: declared; */
                  initial-value: url("/*;");
                  depends-on: --a /* note */ --b;
                }"#,
        );
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "--example");
        assert_eq!(
            definitions[0].activation,
            CustomPropertyActivation::Referenced
        );
        assert_eq!(definitions[0].groups, [] as [LeanStr; 0]);
        assert_eq!(definitions[0].dependencies, ["--a", "--b"]);
        assert_eq!(
            definitions[0].body,
            "syntax: \"*\";
initial-value: url(\"/*;\");"
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn activation_truth_table_and_authored_names_are_explicit() {
        for declared in [false, true] {
            for referenced in [false, true] {
                assert_eq!(
                    CustomPropertyActivation::Declared.is_active(declared, referenced),
                    declared
                );
                assert_eq!(
                    CustomPropertyActivation::Referenced.is_active(declared, referenced),
                    referenced
                );
                assert_eq!(
                    CustomPropertyActivation::Either.is_active(declared, referenced),
                    declared || referenced
                );
            }
        }
        assert_eq!(CustomPropertyActivation::parse("unknown"), None);
        assert_eq!(
            extract_authored_property_names(
                "@property --x {syntax:'*';} @property nope {syntax:'*';}"
            ),
            BTreeSet::from(["--x".into()])
        );
        assert_eq!(
            parse_property_definitions("@stillwind-property nope {syntax:'*';}"),
            [] as [CustomPropertyDefinition; 0]
        );
    }
}
