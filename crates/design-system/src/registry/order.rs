//! Authored cascade positions for arbitrary-property candidates.
//!
//! An arbitrary property such as `[mask-type:luminance]` has no utility definition to
//! inherit a position from, so the registry states positions explicitly. Statements
//! share one ordering space with utility definitions: the position is the directive's
//! byte offset in the registry CSS, which is what lets a property sort before or after
//! a utility that was authored between two statements.

use std::collections::BTreeMap;

use lean_string::LeanStr;

use stillwind_css::{
    decode_css_escapes, extract_named_blocks, extract_named_statements, extract_order_statements,
    whitespace_separated_ranges,
};

use super::variants::parse_variant_name;

/// First registration offsets in the import-expanded stylesheet. Each name keeps its
/// slot across redefinitions, including a redefinition after an undefinition of that
/// name.
///
/// Only definitions that can register a name reserve a slot: `@variant` blocks must
/// carry an `@slot`, and `@custom-variant` also registers from its statement form. The
/// name is the header's first CSS component, decoded as the definition parser of the
/// directive decodes it.
pub(crate) fn registration_positions(css: &str, directives: &[&str]) -> BTreeMap<LeanStr, usize> {
    let mut positions = BTreeMap::<LeanStr, usize>::new();
    for directive in directives {
        let blocks = extract_named_blocks(css, directive)
            .into_iter()
            .filter(|(_, _, body)| *directive != "@variant" || body.contains("@slot"))
            .map(|(offset, header, _)| (offset, header));
        let statements = if *directive == "@custom-variant" {
            extract_named_statements(css, directive)
        } else {
            Vec::new()
        };
        for (offset, header) in blocks.chain(statements) {
            let Some(range) = whitespace_separated_ranges(&header).into_iter().next() else {
                continue;
            };
            let raw = &header[range];
            let name = if directive.ends_with("utility") {
                decode_css_escapes(raw)
            } else {
                parse_variant_name(raw).map(|(name, _)| name)
            };
            let Some(name) = name else {
                continue;
            };
            positions
                .entry(name)
                .and_modify(|position| *position = (*position).min(offset))
                .or_insert(offset);
        }
    }
    positions
}

/// One authored cascade position: an exact property name or a prefix pattern ending in
/// `*`, plus the byte offset the directive was authored at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitraryPropertyOrder {
    /// Property name, or a prefix pattern with a trailing `*`.
    pub pattern: LeanStr,
    /// Byte offset of the directive in the registry CSS.
    pub position: usize,
}

/// Parses explicit cascade positions for arbitrary-property candidates. Patterns use an
/// exact property name or a trailing `*` prefix wildcard.
///
/// A statement whose pattern contains whitespace is skipped rather than guessed at, and
/// the returned order is source order, never a counter.
#[must_use]
pub fn parse_arbitrary_property_order(css: &str) -> Vec<ArbitraryPropertyOrder> {
    extract_order_statements(css, "@stillwind-arbitrary-property")
        .into_iter()
        .map(|(position, pattern)| ArbitraryPropertyOrder { pattern, position })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_positions_preserve_whitespace_inside_css_name_escapes() {
        let css = r"@utility probe-\31 x {color:red;} @utility probe-\32 y {color:blue;}";
        let positions = registration_positions(css, &["@utility"]);
        assert_eq!(positions.get("probe-1x"), Some(&0));
        assert_eq!(
            positions.get("probe-2y"),
            Some(&css.find("@utility probe-\\32").unwrap())
        );
    }

    use crate::registry::utilities::parse_stillwind_utility_definitions;

    #[test]
    fn shares_source_order_between_utilities_and_arbitrary_properties() {
        let css = "@stillwind-arbitrary-property margin;\
                       @stillwind-utility m-* { margin:--value([length]); }\
                       @stillwind-arbitrary-property --tw-*;\
                       @stillwind-arbitrary-property *;";
        let utility = parse_stillwind_utility_definitions(css)
            .into_iter()
            .next()
            .expect("utility");
        let properties = parse_arbitrary_property_order(css);

        assert_eq!(properties.len(), 3);
        assert!(properties[0].position < utility.position.unwrap());
        assert!(utility.position.unwrap() < properties[1].position);
        assert_eq!(properties[1].pattern, "--tw-*");
        assert_eq!(properties[2].pattern, "*");
    }

    #[test]
    fn rejects_whitespace_patterns_and_keeps_source_offsets() {
        let css = "@stillwind-arbitrary-property margin; @stillwind-arbitrary-property --x-*; @stillwind-arbitrary-property two words;";
        let orders = parse_arbitrary_property_order(css);
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].position, 0);
        assert_eq!(
            orders[1].position,
            css.find("@stillwind-arbitrary-property --x-*").unwrap()
        );
        assert_eq!(orders[1].pattern, "--x-*");
    }
}
