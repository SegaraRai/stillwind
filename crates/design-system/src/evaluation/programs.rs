//! Parses utility bodies and declaration branches once for an immutable theme.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::PropertyRequirementError;

use super::utilities::{PreparedDeclarations, PreparedUtilityBody};
use crate::ThemeIndex;

/// Parsed syntax shared by registered utilities with identical body text.
/// Invalid requirements remain stored as errors until a matching utility is evaluated.
#[derive(Debug)]
pub(crate) struct UtilityPrograms {
    bodies: BTreeMap<LeanStr, Arc<PreparedBody>>,
    declarations: BTreeMap<LeanStr, Arc<PreparedDeclarations>>,
}

/// A prepared utility body, or the malformed requirement that prevents its preparation.
type PreparedBody = Result<Option<PreparedUtilityBody>, PropertyRequirementError>;

impl UtilityPrograms {
    /// Parses the bodies of the Tailwind-style and Stillwind definitions using
    /// effective theme values. The result owns its syntax and does not retain the
    /// theme.
    ///
    /// `base` must be the programs of an index with the same tokens. Its entries are
    /// shared, so only the bodies that `base` lacks are parsed.
    pub(crate) fn new(theme: &ThemeIndex, base: Option<&Self>) -> Self {
        let (mut bodies, mut declarations) = base.map_or_else(
            || (BTreeMap::new(), BTreeMap::new()),
            |base| (base.bodies.clone(), base.declarations.clone()),
        );
        for utility in theme.utilities().iter().chain(theme.registry().utilities()) {
            bodies.entry(utility.body.clone()).or_insert_with(|| {
                let body = PreparedUtilityBody::parse(&utility.body);
                if let Ok(Some(body)) = &body {
                    for text in body.declaration_groups() {
                        declarations
                            .entry(text.to_lean_str())
                            .or_insert_with(|| Arc::new(PreparedDeclarations::parse(text, theme)));
                    }
                }
                Arc::new(body)
            });
        }
        Self {
            bodies,
            declarations,
        }
    }

    /// Returns the prepared form of a utility body, or `None` for a body that no
    /// definition of the theme has. The inner result reports a malformed property
    /// requirement, and `Ok(None)` a body that cannot be prepared.
    pub(in crate::evaluation) fn body(&self, body: &str) -> Option<&PreparedBody> {
        self.bodies.get(body).map(Arc::as_ref)
    }

    /// Returns the prepared declarations of one declaration group of a prepared body,
    /// or `None` for text that no prepared body contains.
    pub(in crate::evaluation) fn declarations(&self, body: &str) -> Option<&PreparedDeclarations> {
        self.declarations.get(body).map(Arc::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::theme::ThemeBuilder;
    use crate::{CustomUtility, UtilityRole, resolve_utility};

    #[test]
    fn resolution_retains_the_first_malformed_project_requirement() {
        let authored = ThemeBuilder {
            utilities: ["tone-*", "tone-a"]
                .into_iter()
                .map(|name| CustomUtility {
                    name: name.into(),
                    body: "@stillwind-require-properties invalid; color:red;".into(),
                    position: None,
                    tw_sort: None,
                    role: UtilityRole::default(),
                    plugin: false,
                })
                .map(Arc::new)
                .collect(),
            ..Default::default()
        }
        .finish();
        let actual = resolve_utility("tone-a", &authored).unwrap_err();
        assert_eq!(actual.name, "tone-a");
    }
}
