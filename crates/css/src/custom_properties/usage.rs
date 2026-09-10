use std::collections::BTreeSet;

use lightningcss::error::{Error, ParserError};
use lightningcss::properties::Property;
use lightningcss::properties::custom::Variable;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use super::references::custom_property_references;
use crate::blocks::{strip_blocks, strip_css_comments};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CustomPropertyUsage {
    pub declared: BTreeSet<String>,
    pub referenced: BTreeSet<String>,
    pub declaration_properties: BTreeSet<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("CSS parse error: {0}")]
pub struct PropertyUsageError(#[source] pub Error<ParserError<'static>>);

/// Returns custom properties used by declarations and values, excluding names
/// that occur only inside ordinary authored `@property` registration blocks.
#[must_use]
pub fn used_custom_properties(css: &str) -> BTreeSet<String> {
    custom_property_references(&strip_css_comments(&strip_blocks(css, "@property")))
}

/// Returns only custom properties consumed through `var(...)` references.
/// Definitions such as `--tw-shadow: ...` are intentionally excluded; the
/// caller can use this dependency set to decide which registered properties
/// must be emitted without knowing any framework-specific property families.
#[must_use]
pub fn referenced_custom_properties(css: &str) -> BTreeSet<String> {
    let css = strip_css_comments(&strip_blocks(css, "@property"));
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("var(") {
        let start = cursor + relative + 4;
        let Some(relative_name) = css[start..].find("--") else {
            cursor = start;
            continue;
        };
        let name_start = start + relative_name;
        let end = css[name_start..]
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .last()
            .map_or(name_start + 2, |(offset, character)| {
                name_start + offset + character.len_utf8()
            });
        if end > name_start + 2 {
            references.insert(css[name_start..end].to_owned());
        }
        cursor = end.max(start);
    }
    references
}

/// Parses emitted CSS and classifies custom-property declarations and
/// `var()` references using Lightning CSS's syntax tree.
///
/// This is the activation input for compiler-owned `@property` definitions;
/// registration policy remains declarative metadata rather than property-name
/// knowledge in the compiler.
///
/// # Errors
///
/// Returns an error when the emitted stylesheet cannot be parsed, including
/// with Lightning CSS error recovery enabled.
pub fn custom_property_usage(css: &str) -> Result<CustomPropertyUsage, PropertyUsageError> {
    struct UsageVisitor {
        usage: CustomPropertyUsage,
    }

    impl<'i> Visitor<'i> for UsageVisitor {
        type Error = std::convert::Infallible;

        fn visit_types(&self) -> VisitTypes {
            visit_types!(RULES | PROPERTIES | TOKENS | FUNCTIONS | VARIABLES)
        }

        fn visit_property(&mut self, property: &mut Property<'i>) -> Result<(), Self::Error> {
            self.usage
                .declaration_properties
                .insert(property.property_id().name().to_owned());
            if let Property::Custom(property) = property {
                let name = property.name.as_ref();
                if name.starts_with("--") {
                    self.usage.declared.insert(name.to_owned());
                }
            }
            property.visit_children(self)
        }

        fn visit_variable(&mut self, variable: &mut Variable<'i>) -> Result<(), Self::Error> {
            self.usage
                .referenced
                .insert(variable.name.ident.0.as_ref().to_owned());
            variable.visit_children(self)
        }
    }

    let mut stylesheet = StyleSheet::parse(
        css,
        ParserOptions {
            error_recovery: true,
            ..ParserOptions::default()
        },
    )
    .map_err(|error| PropertyUsageError(error.into_owned()))?;
    let mut visitor = UsageVisitor {
        usage: CustomPropertyUsage::default(),
    };
    match stylesheet.visit(&mut visitor) {
        Ok(()) => {}
        Err(error) => match error {},
    }
    Ok(visitor.usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_custom_property_usage_from_the_css_ast() {
        let usage = custom_property_usage(
            "@layer utilities { .example { --declared: var(--referenced, var(--fallback)); color: var(--color); scroll-snap-type: x var(--snap); transform: var(--transform,) } }",
        )
        .unwrap();

        assert_eq!(usage.declared, BTreeSet::from(["--declared".to_owned()]));
        assert!(usage.declaration_properties.contains("color"));
        assert!(usage.declaration_properties.contains("scroll-snap-type"));
        assert_eq!(
            usage.referenced,
            BTreeSet::from([
                "--color".to_owned(),
                "--fallback".to_owned(),
                "--referenced".to_owned(),
                "--snap".to_owned(),
                "--transform".to_owned(),
            ])
        );
    }

    #[test]
    fn extracts_only_var_custom_property_dependencies() {
        let css = "--unused:red;--used:var(--source, var(--fallback));@property --ignored { syntax: '*' }";
        assert_eq!(
            referenced_custom_properties(css),
            BTreeSet::from(["--source".to_owned(), "--fallback".to_owned()])
        );
    }
}
