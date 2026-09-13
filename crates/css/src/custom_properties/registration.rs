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
//! registrations never shadow them.

use std::collections::BTreeSet;

use crate::blocks::{extract_named_blocks, strip_css_comments};

/// One parsed `@stillwind-property` registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPropertyDefinition {
    /// Property name including the leading `--`.
    pub name: String,
    /// Standard `@property` descriptors, one per line, with the activation
    /// metadata removed. Emitted verbatim inside the generated block.
    pub body: String,
    /// When the compiler should emit this registration.
    pub activation: CustomPropertyActivation,
    /// Custom properties this registration needs; each dependency is itself a
    /// candidate for emission.
    pub dependencies: Vec<String>,
    /// Free-form group names, used to activate registrations together.
    pub groups: Vec<String>,
    /// Ordinary property names that activate this registration when the output
    /// declares a value for them.
    pub activation_properties: Vec<String>,
}

/// One compiler-owned custom-property registration declared with
/// `@stillwind-property`. Ordinary authored `@property` rules are never added
/// to this registry and remain untouched in the output stylesheet.
///
/// The default is `Referenced`: a registration is emitted only when something
/// reads the property, which keeps unused descriptors out of the output.
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
    /// Parses the `activation:` descriptor value; unknown values are ignored by
    /// the caller, which keeps the default in place.
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
    /// Both flags come from an emitted stylesheet, so activation is decided
    /// after generation rather than predicted from configuration.
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

/// Parses every `@stillwind-property` block in source order.
///
/// Comments are stripped first, so a commented-out registration is not parsed.
/// Blocks without a `--` name are skipped rather than reported.
pub fn extract_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_named_blocks(&strip_css_comments(css), "@stillwind-property")
        .into_iter()
        .filter_map(|(header, body)| {
            let name = header.split_whitespace().next()?;
            if !name.starts_with("--") {
                return None;
            }
            let mut activation = CustomPropertyActivation::default();
            let mut dependencies = Vec::new();
            let mut groups = Vec::new();
            let mut activation_properties = Vec::new();
            let mut standard_descriptors = Vec::new();
            for descriptor in body.split(';') {
                let trimmed = descriptor.trim();
                if let Some(value) = trimmed.strip_prefix("activation:") {
                    if let Some(parsed) = CustomPropertyActivation::parse(value) {
                        activation = parsed;
                    }
                } else if let Some(value) = trimmed.strip_prefix("depends-on:") {
                    dependencies.extend(value.split_whitespace().map(str::to_owned));
                } else if let Some(value) = trimmed.strip_prefix("group:") {
                    groups.extend(value.split_whitespace().map(str::to_owned));
                } else if let Some(value) = trimmed.strip_prefix("activate-on:") {
                    activation_properties.extend(value.split_whitespace().map(str::to_owned));
                } else if !trimmed.is_empty() {
                    standard_descriptors.push(format!("{trimmed};"));
                }
            }
            Some(CustomPropertyDefinition {
                name: name.to_owned(),
                body: standard_descriptors.join("\n"),
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
/// Names are gathered so the compiler can avoid emitting a registration for a
/// property the author already registers; nothing is removed from the output.
pub fn extract_authored_property_names(css: &str) -> BTreeSet<String> {
    extract_named_blocks(&strip_css_comments(css), "@property")
        .into_iter()
        .filter_map(|(header, _)| {
            let name = header.split_whitespace().next()?;
            name.starts_with("--").then(|| name.to_owned())
        })
        .collect()
}

/// Parses compiler-owned custom-property registrations (`@stillwind-property`).
#[must_use]
pub fn parse_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_property_definitions(css)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(parse_property_definitions("@stillwind-property nope {syntax:'*';}").is_empty());
    }
}
