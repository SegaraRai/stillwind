//! Authored cascade positions for arbitrary-property candidates.
//!
//! An arbitrary property such as `[mask-type:luminance]` has no utility definition to
//! inherit a position from, so the registry states positions explicitly. Statements
//! share one ordering space with utility definitions: the position is the directive's
//! byte offset in the registry CSS, which is what lets a property sort before or after
//! a utility that was authored between two statements.

use stillwind_css::blocks::extract_order_statements;

/// One authored cascade position: an exact property name or a prefix pattern ending in
/// `*`, plus the byte offset the directive was authored at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinArbitraryPropertyOrder {
    /// Property name, or a prefix pattern with a trailing `*`.
    pub pattern: String,
    /// Byte offset of the directive in the registry CSS.
    pub builtin_order: usize,
}

/// Parses explicit cascade positions for arbitrary-property candidates. Patterns use an
/// exact property name or a trailing `*` prefix wildcard.
///
/// A statement whose pattern contains whitespace is skipped rather than guessed at, and
/// the returned order is source order, never a counter.
#[must_use]
pub fn parse_builtin_arbitrary_property_order(css: &str) -> Vec<BuiltinArbitraryPropertyOrder> {
    extract_order_statements(css, "@stillwind-arbitrary-property")
        .into_iter()
        .map(|(builtin_order, pattern)| BuiltinArbitraryPropertyOrder {
            pattern,
            builtin_order,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::utilities::parse_builtin_utility_definitions;

    #[test]
    fn shares_source_order_between_utilities_and_arbitrary_properties() {
        let css = "@stillwind-arbitrary-property margin;\
                       @stillwind-utility m-* { margin:--value([length]); }\
                       @stillwind-arbitrary-property --tw-*;\
                       @stillwind-arbitrary-property *;";
        let utility = parse_builtin_utility_definitions(css)
            .into_iter()
            .next()
            .expect("utility");
        let properties = parse_builtin_arbitrary_property_order(css);

        assert_eq!(properties.len(), 3);
        assert!(properties[0].builtin_order < utility.builtin_order.unwrap());
        assert!(utility.builtin_order.unwrap() < properties[1].builtin_order);
        assert_eq!(properties[1].pattern, "--tw-*");
        assert_eq!(properties[2].pattern, "*");
    }

    #[test]
    fn rejects_whitespace_patterns_and_keeps_source_offsets() {
        let css = "@stillwind-arbitrary-property margin; @stillwind-arbitrary-property --x-*; @stillwind-arbitrary-property two words;";
        let orders = parse_builtin_arbitrary_property_order(css);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].builtin_order, 0);
        assert_eq!(
            orders[1].builtin_order,
            css.find("@stillwind-arbitrary-property --x-*").unwrap()
        );
        assert_eq!(orders[1].pattern, "--x-*");
    }
}
