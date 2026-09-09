/// One curated CSS property definition extracted from the CSSWG Web Living
/// Standards by W3C Webref. The generated table is the source of truth for
/// supported property names and their formal value syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WebrefPropertySyntax {
    pub name: &'static str,
    pub syntax: &'static str,
    pub href: &'static str,
}

include!("../../assets/webref-properties.rs");

pub(crate) fn webref_property(name: &str) -> Option<&'static WebrefPropertySyntax> {
    WEBREF_PROPERTIES
        .iter()
        .find(|property| property.name.eq_ignore_ascii_case(name))
}

/// Returns the CSSWG Web Living Standard's formal value syntax for a CSS
/// property, when Webref publishes one.
///
/// This intentionally exposes only the grammar text. Consumers use it to
/// audit their own value domains; parsing it is not part of Stillwind's public
/// utility syntax.
#[must_use]
pub fn csswg_formal_syntax(property: &str) -> Option<&'static str> {
    webref_property(property).map(|definition| definition.syntax)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_lookup_is_case_insensitive_and_rejects_unknown_names() {
        assert_eq!(webref_property("COLOR"), webref_property("color"));
        assert!(csswg_formal_syntax("color").unwrap().contains("color"));
        assert_eq!(csswg_formal_syntax("not-a-real-property"), None);
    }
}
