use std::collections::BTreeSet;

use crate::blocks::{extract_named_blocks, strip_css_comments};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPropertyDefinition {
    pub name: String,
    pub body: String,
    pub activation: CustomPropertyActivation,
    pub dependencies: Vec<String>,
    pub groups: Vec<String>,
    pub activation_properties: Vec<String>,
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
    pub(crate) fn parse(value: &str) -> Option<Self> {
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

pub(crate) fn extract_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
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

pub(crate) fn extract_authored_property_names(css: &str) -> BTreeSet<String> {
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
