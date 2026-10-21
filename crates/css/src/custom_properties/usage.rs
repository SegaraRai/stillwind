//! Which custom properties an emitted stylesheet declares and consumes.
//!
//! Compiler-owned `@property` registrations are emitted only when the output actually
//! needs them, and that decision is made here rather than from knowledge of
//! framework-specific property families. [`custom_property_usage`] walks the Lightning
//! CSS syntax tree.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};
use lightningcss::declaration::DeclarationBlock;
use lightningcss::error::{Error, ParserError};
use lightningcss::properties::Property;
use lightningcss::properties::custom::Variable;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

/// Failure to parse an emitted stylesheet, even with error recovery enabled.
#[derive(Debug, thiserror::Error)]
#[error("CSS parse error: {0}")]
pub struct PropertyUsageError(#[source] pub Error<ParserError<'static>>);

/// Custom properties an emitted stylesheet declares, consumes, and declares values for.
///
/// The sets are independent: a name may be referenced without being declared, and
/// `declaration_properties` records the property each declaration belongs to, which is
/// how activation rules matching property names are evaluated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CustomPropertyUsage {
    /// Custom properties declared in a declaration block, such as `--x: red`.
    pub declared: BTreeSet<LeanStr>,
    /// Custom properties read through `var()`, including nested fallbacks.
    pub referenced: BTreeSet<LeanStr>,
    /// Names of the ordinary properties that appear in the stylesheet.
    pub declaration_properties: BTreeSet<LeanStr>,
}

impl CustomPropertyUsage {
    /// Adds usage from a declaration list without a selector or surrounding braces.
    /// Existing usage is retained, including when the input is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if Lightning CSS cannot parse the declarations with error
    /// recovery enabled. On error, the existing usage is unchanged.
    pub fn add_declarations(&mut self, css: &str) -> Result<(), PropertyUsageError> {
        let mut declarations = DeclarationBlock::parse_string(
            css,
            ParserOptions {
                error_recovery: true,
                ..ParserOptions::default()
            },
        )
        .map_err(|error| {
            PropertyUsageError(Error::from(error, std::string::String::new()).into_owned())
        })?;
        match declarations.visit(self) {
            Ok(()) => Ok(()),
            Err(error) => match error {},
        }
    }
}

impl<'i> Visitor<'i> for CustomPropertyUsage {
    type Error = std::convert::Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(RULES | PROPERTIES | TOKENS | FUNCTIONS | VARIABLES)
    }

    fn visit_property(&mut self, property: &mut Property<'i>) -> Result<(), Self::Error> {
        self.declaration_properties
            .insert(property.property_id().name().to_lean_str());
        if let Property::Custom(property) = property {
            let name = property.name.as_ref();
            if name.starts_with("--") {
                self.declared.insert(name.to_lean_str());
            }
        }
        property.visit_children(self)
    }

    fn visit_variable(&mut self, variable: &mut Variable<'i>) -> Result<(), Self::Error> {
        self.referenced
            .insert(variable.name.ident.0.as_ref().to_lean_str());
        variable.visit_children(self)
    }
}

/// Parses emitted CSS and classifies custom-property declarations and
/// `var()` references using Lightning CSS's syntax tree.
///
/// This is the activation input for compiler-owned `@property` definitions;
/// registration policy remains declarative metadata rather than property-name
/// knowledge in the compiler.
///
/// Parsing uses error recovery, so a nearly-valid stylesheet still produces
/// usage; only a stylesheet Lightning CSS refuses entirely is an error.
///
/// # Errors
///
/// Returns an error when the emitted stylesheet cannot be parsed, including
/// with Lightning CSS error recovery enabled.
pub fn custom_property_usage(css: &str) -> Result<CustomPropertyUsage, PropertyUsageError> {
    let mut stylesheet = StyleSheet::parse(
        css,
        ParserOptions {
            error_recovery: true,
            ..ParserOptions::default()
        },
    )
    .map_err(|error| PropertyUsageError(error.into_owned()))?;
    let mut visitor = CustomPropertyUsage::default();
    match stylesheet.visit(&mut visitor) {
        Ok(()) => {}
        Err(error) => {
            match error {};
        }
    }
    Ok(visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_usage_preserves_css_classification_and_existing_demand() {
        let css = ".existing{width:var(--existing)}";
        let declarations = r#"--theme:var(--source,var(--fallback)); color:var(--color)!important; content:"var(--ignored)"; --block:{nested:var(--nested)}; /* var(--comment) */"#;
        let mut usage = custom_property_usage(css).unwrap();
        usage.add_declarations(declarations).unwrap();
        assert_eq!(
            usage,
            custom_property_usage(&format_args!("{css}:root{{{declarations}}}").to_lean_str())
                .unwrap()
        );
        assert_eq!(
            usage.referenced,
            BTreeSet::from(
                [
                    "--existing",
                    "--source",
                    "--fallback",
                    "--color",
                    "--nested",
                ]
                .map(LeanStr::from)
            )
        );
        assert_eq!(
            usage.declared,
            BTreeSet::from(["--theme", "--block"].map(LeanStr::from))
        );
        let before = usage.clone();
        usage.add_declarations("").unwrap();
        assert_eq!(usage, before);
    }

    #[test]
    fn classifies_custom_property_usage_from_the_css_ast() {
        let usage = custom_property_usage(
            "@layer utilities { .example { --declared: var(--referenced, var(--fallback)); color: var(--color); scroll-snap-type: x var(--snap); transform: var(--transform,) } }",
        )
        .unwrap();

        assert_eq!(usage.declared, BTreeSet::from(["--declared".to_lean_str()]));
        assert!(usage.declaration_properties.contains("color"));
        assert!(usage.declaration_properties.contains("scroll-snap-type"));
        assert_eq!(
            usage.referenced,
            BTreeSet::from([
                "--color".to_lean_str(),
                "--fallback".to_lean_str(),
                "--referenced".to_lean_str(),
                "--snap".to_lean_str(),
                "--transform".to_lean_str(),
            ])
        );
    }
}
