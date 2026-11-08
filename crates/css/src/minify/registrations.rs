//! Custom properties that `@property` registers with a number syntax.

use std::collections::HashSet;

use lightningcss::rules::{CssRule, CssRuleList};
use lightningcss::values::syntax::{SyntaxComponentKind, SyntaxString};

/// Returns the names of the custom properties that an `@property` rule in `rules`
/// registers with `<number>` as its only data type, such as `<number>` or `<number>+`.
///
/// A registered `<number>` property computes `calc(98 / 100)` to `.98`, so a folded
/// literal gives the same computed value. A syntax that also allows `<integer>`, as in
/// `<integer> | <number>`, is excluded, because the `<integer>` alternative rounds a
/// `calc()` result. Rules inside `@layer` blocks are included.
pub(super) fn number_properties(rules: &CssRuleList<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    collect(rules, &mut names);
    names
}

fn collect(rules: &CssRuleList<'_>, names: &mut HashSet<String>) {
    for rule in &rules.0 {
        match rule {
            CssRule::Property(property) => {
                if let SyntaxString::Components(components) = &property.syntax
                    && !components.is_empty()
                    && components
                        .iter()
                        .all(|component| component.kind == SyntaxComponentKind::Number)
                {
                    names.insert(property.name.0.to_string());
                }
            }
            CssRule::LayerBlock(layer) => {
                collect(&layer.rules, names);
            }
            _ => {}
        }
    }
}
