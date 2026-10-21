//! Marker classes: class names that a candidate adds to an element as written.
//!
//! Generated class names cannot satisfy a selector that names another class, such as
//! `.group:hover &` or `.prose :where(...)`. A marker class keeps its written name, so
//! separately compiled `style()` calls agree on the class that such a selector matches.
//!
//! `@stillwind-marker name;` declares a marker explicitly, and `@stillwind-marker name
//! (modifiable);` declares one that also accepts a named modifier, as `group/menu`
//! does. Every class that a utility or variant definition selects by name is also a
//! marker. This includes a utility's own name when its body writes it, as
//! `:where(.prose > :first-child)` does inside `prose`. Only the literal class
//! satisfies such a selector. `&` names no class.
//!
//! `@stillwind-marker-source "css";` supplies rules whose selected classes are markers
//! without defining anything. A host passes the CSS that a plugin would generate for
//! candidates that no one has requested yet, so a class is a marker before any module
//! requests the candidate that selects it. The CSS is a string so that no other
//! directive scanner reads the rules inside it, and the statement emits nothing.
//!
//! This module records the markers; candidate classification lives with variant
//! compatibility in the evaluator.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanStr;

use stillwind_css::{
    CssNode, CssNodeKind, extract_named_statements, parse_structure, selector_class_names,
    split_top_level,
};
use stillwind_syntax::{Utility, Value, parse_candidate};

use super::{CustomUtility, CustomVariant};

/// Candidate forms that add a marker class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MarkerForm {
    /// Only the exact name adds the class.
    Exact,
    /// The bare name and the name with a named modifier, such as `group/menu`, each add
    /// the class written in the candidate.
    Modifiable,
}

/// A statement or definition that makes a class a marker.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MarkerOrigin {
    /// A `@stillwind-marker` statement declares the class.
    Declared,
    /// A rule in a `@stillwind-marker-source` string selects the class.
    Source,
    /// A selector in the body of the utility with this definition name selects the
    /// class.
    Utility(LeanStr),
    /// The selector or body of the variant with this definition name selects the class.
    Variant(LeanStr),
}

/// Marker class names, the candidate forms that add them, and their origins.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MarkerClasses {
    /// Decoded class name, which contains a `/` when a definition names a class such as
    /// `.foo\/bar`. A name has one entry.
    entries: BTreeMap<LeanStr, MarkerEntry>,
}

/// The form and origins that [`MarkerClasses`] records for one class name.
#[derive(Clone, Debug, Eq, PartialEq)]
struct MarkerEntry {
    /// [`MarkerForm::Modifiable`] wins when origins disagree.
    form: MarkerForm,
    /// Every origin of the class. It is never empty.
    origins: BTreeSet<MarkerOrigin>,
}

impl MarkerClasses {
    /// Returns the form of the marker class `name`, or `None` when `name` is not one.
    pub fn get(&self, name: &str) -> Option<MarkerForm> {
        self.entries.get(name).map(|entry| entry.form)
    }

    /// Returns every marker class name and its form, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, MarkerForm)> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.as_str(), entry.form))
    }

    /// Returns the origins of the marker class `name` in [`MarkerOrigin`] order. The
    /// iterator is empty when `name` is not a marker class.
    pub fn origins(&self, name: &str) -> impl Iterator<Item = &MarkerOrigin> {
        self.entries
            .get(name)
            .into_iter()
            .flat_map(|entry| entry.origins.iter())
    }

    /// Collects the markers that `css` declares with `@stillwind-marker`, and the
    /// classes that `utilities`, `variants`, and the `@stillwind-marker-source`
    /// statements of `css` select by name.
    ///
    /// A `@stillwind-marker` statement with an argument other than `(modifiable)`
    /// declares nothing, and a `@stillwind-marker-source` statement whose argument is
    /// not one string supplies nothing. Other selected classes follow
    /// [`Self::selected_by`].
    pub(crate) fn collect<'a>(
        css: &str,
        utilities: impl IntoIterator<Item = &'a CustomUtility>,
        variants: impl IntoIterator<Item = &'a CustomVariant>,
    ) -> Self {
        let mut markers = Self::selected_by(utilities, variants);
        // Each scan tokenizes all of `css`, which includes the whole package CSS.
        if !css.contains("@stillwind-marker") {
            return markers;
        }
        for (_, header) in extract_named_statements(css, "@stillwind-marker") {
            if let Some((name, form)) = parse_marker_statement(&header) {
                markers.insert(name, form, MarkerOrigin::Declared);
            }
        }
        if css.contains("@stillwind-marker-source") {
            for (_, header) in extract_named_statements(css, "@stillwind-marker-source") {
                let mut parser = Parser::new(&header);
                if let Ok(source) = parser.expect_string().map(ToString::to_string)
                    && parser.is_exhausted()
                {
                    for name in body_class_names(&source) {
                        markers.insert_selected(name, &MarkerOrigin::Source);
                    }
                }
            }
        }
        markers
    }

    /// Collects the classes that `utilities` and `variants` select by name, including a
    /// utility's own name when its body writes it.
    ///
    /// A selected class whose name cannot be written as a variant-free candidate, such
    /// as `hover:x` or `-x`, is skipped, because no candidate could add it.
    pub(crate) fn selected_by<'a>(
        utilities: impl IntoIterator<Item = &'a CustomUtility>,
        variants: impl IntoIterator<Item = &'a CustomVariant>,
    ) -> Self {
        let mut markers = Self::default();
        for utility in utilities {
            let origin = MarkerOrigin::Utility(utility.name.clone());
            for name in body_class_names(&utility.body) {
                markers.insert_selected(name, &origin);
            }
        }
        for variant in variants {
            let origin = MarkerOrigin::Variant(variant.name.clone());
            for branch in split_top_level(&variant.selector, |token| matches!(token, Token::Comma))
            {
                if !branch.trim_start().starts_with('@') {
                    for name in selector_class_names(branch) {
                        markers.insert_selected(name, &origin);
                    }
                }
            }
            for name in body_class_names(&variant.body) {
                markers.insert_selected(name, &origin);
            }
        }
        markers
    }

    fn insert(&mut self, name: LeanStr, form: MarkerForm, origin: MarkerOrigin) {
        let entry = self.entries.entry(name).or_insert_with(|| MarkerEntry {
            form,
            origins: BTreeSet::new(),
        });
        entry.form = entry.form.max(form);
        entry.origins.insert(origin);
    }

    fn insert_selected(&mut self, name: LeanStr, origin: &MarkerOrigin) {
        if candidate_name(&name) {
            self.insert(name, MarkerForm::Exact, origin.clone());
        }
    }
}

/// Parses the text after `@stillwind-marker` into a decoded name and its form, or
/// returns `None` when the text is not an identifier optionally followed by
/// `(modifiable)`.
fn parse_marker_statement(header: &str) -> Option<(LeanStr, MarkerForm)> {
    let mut parser = Parser::new(header);
    let name = LeanStr::from(parser.expect_ident().ok()?.as_ref());
    let form = if parser.is_exhausted() {
        MarkerForm::Exact
    } else {
        parser.expect_parenthesis_block().ok()?;
        parser
            .parse_nested_block(|inner| {
                inner.expect_ident_matching("modifiable")?;
                inner.expect_exhausted()?;
                Ok::<_, ParseError<()>>(())
            })
            .ok()?;
        MarkerForm::Modifiable
    };
    parser.is_exhausted().then_some((name, form))
}

/// Returns the class names that the style rule selectors of a definition body select,
/// including rules nested in at-rules. At-rule preludes and declarations are not
/// selectors and are skipped.
fn body_class_names(body: &str) -> Vec<LeanStr> {
    fn visit(body: &str, nodes: &[CssNode], names: &mut Vec<LeanStr>) {
        for node in nodes {
            if let CssNodeKind::StyleRule { selector, .. } = &node.kind {
                names.extend(selector_class_names(&body[selector.clone()]));
            }
            visit(body, node.children(), names);
        }
    }
    let mut names = Vec::new();
    if body.contains('.') || body.contains("class") {
        visit(body, &parse_structure(body), &mut names);
    }
    names
}

/// Whether a candidate without variants, `!`, or `-` can spell the class `name`.
fn candidate_name(name: &str) -> bool {
    parse_candidate(name).is_some_and(|candidate| {
        candidate.variants.is_empty()
            && !candidate.important
            && !candidate.negative
            && matches!(
                candidate.utility,
                Utility::Named {
                    modifier: None | Some(Value::Named(_)),
                    ..
                }
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{utility_definitions, variant_definitions};

    fn collect(css: &str) -> Vec<(String, MarkerForm)> {
        let utilities = utility_definitions(css);
        let variants = variant_definitions(css);
        MarkerClasses::collect(css, &utilities, &variants)
            .iter()
            .map(|(name, form)| (name.to_owned(), form))
            .collect()
    }

    #[test]
    fn reads_marker_statements_and_their_forms() {
        assert_eq!(
            collect(
                r"@stillwind-marker group (modifiable); @stillwind-marker x;
                 @stillwind-marker y (other); @stillwind-marker x (modifiable);
                 @stillwind-marker a\/b;"
            ),
            [
                ("a/b".into(), MarkerForm::Exact),
                ("group".into(), MarkerForm::Modifiable),
                ("x".into(), MarkerForm::Modifiable),
            ]
        );
    }

    #[test]
    fn collects_classes_selected_by_utilities_and_variants() {
        assert_eq!(
            collect(
                r#"@utility a { color: red; .b & { color: blue; } }
                @utility b { & .a, &:not([class~="not-c"]) { margin: 0; } }
                @utility d { @media (width >= 40.5rem) { :where(.d) & { content: ".e"; } } }
                @custom-variant dark (&:where(.dark, .dark *));
                @custom-variant open { .is-open & { @slot; } }
                @custom-variant print (@media print);"#
            ),
            [
                ("a".into(), MarkerForm::Exact),
                ("b".into(), MarkerForm::Exact),
                ("d".into(), MarkerForm::Exact),
                ("dark".into(), MarkerForm::Exact),
                ("is-open".into(), MarkerForm::Exact),
                ("not-c".into(), MarkerForm::Exact),
            ]
        );
    }

    #[test]
    fn collects_classes_selected_by_marker_sources_without_defining_them() {
        let css = r#"@stillwind-marker-source ".stack-root &{gap:4px}@property --x{syntax:'*'}";
            @stillwind-marker-source ".a &{color:red}" extra;"#;
        assert_eq!(collect(css), [("stack-root".into(), MarkerForm::Exact)]);
        assert_eq!(utility_definitions(css), []);
    }

    #[test]
    fn skips_classes_that_no_candidate_can_spell() {
        assert_eq!(
            collect(r"@utility a { .hover\:b &, .-c &, .d\/\[e\] & { color: red; } }"),
            [] as [(String, MarkerForm); 0]
        );
        assert_eq!(
            collect(r"@utility a { .b\/c & { color: red; } }"),
            [("b/c".into(), MarkerForm::Exact)]
        );
    }
}
