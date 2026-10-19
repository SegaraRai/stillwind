//! Marker queries describe the class that a candidate adds to an element as written.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;

use stillwind_design_system::{
    DesignSystemRevision, MarkerForm, MarkerOrigin, MarkerUse, ThemeIndex, marker_use,
};

use super::{CandidateContext, DesignSystemQuery};

/// The marker class that one candidate adds, with the statements and definitions that
/// make the class a marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddedMarker {
    /// Decoded class name that the element receives, including a named modifier of a
    /// modifiable marker.
    pub class: LeanStr,
    /// Origins of the marker name in [`MarkerOrigin`] order, without duplicates. For a
    /// modifier of a modifiable marker, these are the origins of the name before `/`.
    pub origins: Vec<MarkerOrigin>,
    /// Snapshot revision the answer was computed from.
    pub revision: DesignSystemRevision,
}

impl DesignSystemQuery {
    /// Returns the marker class that decoded `candidate` adds to the element when it is
    /// written in an input of `context`.
    ///
    /// Only [`CandidateContext::Utility`] inputs add marker classes. The candidate must
    /// have no variants, `!`, or leading `-`, and its modifier must be part of the
    /// marker name or accepted by a modifiable marker. Returns `None` in every other
    /// case, including a candidate that names no marker class. A ref condition of an
    /// enclosing style object also prevents the class, and the caller must check it.
    pub fn added_marker(&self, candidate: &str, context: CandidateContext) -> Option<AddedMarker> {
        if context != CandidateContext::Utility {
            return None;
        }
        let theme = self.context.design_system.theme();
        let MarkerUse::Class(class) = marker_use(candidate, theme)? else {
            return None;
        };
        let class = LeanStr::from(class.as_str());
        let mut origins = marker_origins(&class, theme);
        if origins.is_empty()
            && let Some((name, _)) = class.split_once('/')
        {
            origins = marker_origins(name, theme);
        }
        Some(AddedMarker {
            class,
            origins: origins.into_iter().collect(),
            revision: self.context.design_system.revision(),
        })
    }

    /// Returns the marker names that start with `prefix`, with their forms, from the
    /// stylesheet and the built-ins. A name that both declare has the wider form.
    pub(super) fn marker_names(&self, prefix: &str) -> Vec<(LeanStr, MarkerForm)> {
        let theme = self.context.design_system.theme();
        let mut names = BTreeMap::<LeanStr, MarkerForm>::new();
        for (name, form) in theme
            .markers()
            .iter()
            .chain(theme.registry().markers().iter())
        {
            if name.starts_with(prefix) {
                let entry = names.entry(name.into()).or_insert(form);
                *entry = (*entry).max(form);
            }
        }
        names.into_iter().collect()
    }
}

/// Returns the origins that the stylesheet and the built-ins record for the marker
/// class `name`.
fn marker_origins(name: &str, theme: &ThemeIndex) -> BTreeSet<MarkerOrigin> {
    theme
        .markers()
        .origins(name)
        .chain(theme.registry().markers().origins(name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::CompilerOptions;

    fn query(css: &str) -> DesignSystemQuery {
        let source = css_source(FileId(1), css).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        DesignSystemQuery::new(snapshot, CompilerOptions::default())
    }

    #[test]
    fn describes_the_class_and_the_origins_of_an_added_marker() {
        let query = query(
            "@stillwind-marker menu (modifiable);
            @utility card-title { .card & { color: red; } }
            @custom-variant in-card (.card &);
            @utility card { color: blue; }",
        );
        let marker = query
            .added_marker("card", CandidateContext::Utility)
            .unwrap();
        assert_eq!(marker.class, "card");
        assert_eq!(
            marker.origins,
            [
                MarkerOrigin::Utility("card-title".into()),
                MarkerOrigin::Variant("in-card".into()),
            ]
        );
        let modified = query
            .added_marker("menu/main", CandidateContext::Utility)
            .unwrap();
        assert_eq!(modified.class, "menu/main");
        assert_eq!(modified.origins, [MarkerOrigin::Declared]);
    }

    #[test]
    fn adds_no_marker_in_other_forms_or_inputs() {
        let query = query("@utility card-title { .card & { color: red; } }");
        for candidate in ["hover:card", "!card", "card/x", "card-title", "box"] {
            assert_eq!(
                query.added_marker(candidate, CandidateContext::Utility),
                None,
                "{candidate}"
            );
        }
        assert_eq!(query.added_marker("card", CandidateContext::Document), None);
    }
}
