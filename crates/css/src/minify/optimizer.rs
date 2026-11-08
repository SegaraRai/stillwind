//! The visitor that applies Stillwind rewrites after Lightning CSS has minified a
//! stylesheet.

use std::collections::HashSet;
use std::convert::Infallible;

use lightningcss::media_query::MediaQuery;
use lightningcss::properties::Property;
use lightningcss::properties::custom::{CustomPropertyName, Function, TokenList};
use lightningcss::rules::CssRule;
use lightningcss::rules::supports::SupportsCondition;
use lightningcss::selector::SelectorList;
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use super::queries::{
    optimize_container_condition, optimize_media_query, optimize_supports_condition,
};
use super::rules::{merge_keyframes, remove_duplicate_selectors};
use super::tokens::{optimize_function_arguments, optimize_token_list};

/// Applies the rewrites of the child modules to a stylesheet that Lightning CSS has
/// already minified.
pub(super) struct Optimizer {
    /// Custom properties that the stylesheet registers with a number syntax, from
    /// [`number_properties`](super::registrations::number_properties).
    number_properties: HashSet<String>,
    /// Whether the visitor is inside the value of a property in `number_properties`.
    in_number_property: bool,
}

impl Optimizer {
    /// Creates an optimizer for a stylesheet that registers `number_properties`.
    pub(super) fn new(number_properties: HashSet<String>) -> Self {
        Self {
            number_properties,
            in_number_property: false,
        }
    }
}

impl<'i> Visitor<'i> for Optimizer {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(
            RULES
                | PROPERTIES
                | MEDIA_QUERIES
                | SUPPORTS_CONDITIONS
                | SELECTORS
                | FUNCTIONS
                | TOKENS
        )
    }

    fn visit_rule(&mut self, rule: &mut CssRule<'i>) -> Result<(), Self::Error> {
        match rule {
            CssRule::Container(container) => {
                if let Some(condition) = &mut container.condition {
                    optimize_container_condition(condition);
                }
            }
            CssRule::Keyframes(keyframes) => {
                merge_keyframes(keyframes);
            }
            _ => {}
        }
        rule.visit_children(self)
    }

    fn visit_media_query(&mut self, query: &mut MediaQuery<'i>) -> Result<(), Self::Error> {
        optimize_media_query(query);
        Ok(())
    }

    fn visit_supports_condition(
        &mut self,
        condition: &mut SupportsCondition<'i>,
    ) -> Result<(), Self::Error> {
        optimize_supports_condition(condition);
        Ok(())
    }

    fn visit_selector_list(&mut self, selectors: &mut SelectorList<'i>) -> Result<(), Self::Error> {
        remove_duplicate_selectors(selectors);
        Ok(())
    }

    fn visit_function(&mut self, function: &mut Function<'i>) -> Result<(), Self::Error> {
        optimize_function_arguments(function);
        function.visit_children(self)
    }

    fn visit_property(&mut self, property: &mut Property<'i>) -> Result<(), Self::Error> {
        let registered = matches!(property, Property::Custom(custom)
            if matches!(&custom.name, CustomPropertyName::Custom(name)
                if self.number_properties.contains(name.0.as_ref())));
        self.in_number_property = registered;
        let result = property.visit_children(self);
        self.in_number_property = false;
        result
    }

    fn visit_token_list(&mut self, tokens: &mut TokenList<'i>) -> Result<(), Self::Error> {
        optimize_token_list(&mut tokens.0, self.in_number_property);
        tokens.visit_children(self)
    }
}
