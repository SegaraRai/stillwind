//! Parses utility bodies and declaration branches once for an immutable theme.

use std::collections::BTreeMap;

use stillwind_css::PropertyRequirementError;

use super::utilities::PreparedDeclarations;
use super::utilities::PreparedUtilityBody;
use crate::ThemeIndex;

/// Parsed syntax shared by registered utilities with identical body text.
/// Invalid requirements remain stored as errors until a matching utility is evaluated.
#[derive(Debug)]
pub(crate) struct UtilityPrograms {
    bodies: BTreeMap<String, Result<Option<PreparedUtilityBody>, PropertyRequirementError>>,
    declarations: BTreeMap<String, PreparedDeclarations>,
}

impl UtilityPrograms {
    /// Parses the project, built-in, and animation-option bodies using effective theme
    /// values. The result owns its syntax and does not retain the theme.
    pub(crate) fn new(theme: &ThemeIndex) -> Self {
        let mut bodies = BTreeMap::new();
        let mut declarations = BTreeMap::new();
        for utility in theme
            .utilities()
            .iter()
            .chain(theme.registry().utilities())
            .chain(theme.registry().animation_options())
        {
            bodies.entry(utility.body.clone()).or_insert_with(|| {
                let body = PreparedUtilityBody::parse(&utility.body);
                if let Ok(Some(body)) = &body {
                    for text in body.declaration_groups() {
                        declarations
                            .entry(text.to_owned())
                            .or_insert_with(|| PreparedDeclarations::parse(text, theme));
                    }
                }
                body
            });
        }
        Self {
            bodies,
            declarations,
        }
    }

    pub(crate) fn body(
        &self,
        body: &str,
    ) -> Option<&Result<Option<PreparedUtilityBody>, PropertyRequirementError>> {
        self.bodies.get(body)
    }

    pub(crate) fn declarations(&self, body: &str) -> Option<&PreparedDeclarations> {
        self.declarations.get(body)
    }
}

#[cfg(test)]
mod tests {
    use crate::theme::ThemeBuilder;
    use crate::{CustomUtility, resolve_utility};

    #[test]
    fn resolution_retains_the_first_malformed_project_requirement() {
        let authored = ThemeBuilder {
            utilities: ["tone-*", "tone-a"]
                .into_iter()
                .map(|name| CustomUtility {
                    name: name.into(),
                    body: "@stillwind-require-properties invalid; color:red;".into(),
                    builtin_order: None,
                    tw_sort: None,
                })
                .collect(),
            ..Default::default()
        }
        .finish();
        let actual = resolve_utility("tone-a", &authored).unwrap_err();
        assert_eq!(actual.name, "tone-a");
    }
}
