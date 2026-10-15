//! Variant resolution across the built-in registry.
//!
//! Pins attribute, responsive, selector, media, and structural variants against the
//! same definitions the compiler uses, so a registry change that renames or retypes
//! a variant fails here first.

use lean_string::{LeanString, ToLeanStr};

use super::*;

#[test]
fn resolves_state_attribute_responsive_and_selector_variants() {
    let theme = theme();
    assert_eq!(
        resolve_variant(&parse_candidate("hover:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::SelectorMedia {
            selector: "&:hover".to_lean_str(),
            condition: "(hover: hover)".to_lean_str(),
        })
    );
    assert_eq!(
        resolve_variant(&parse_candidate("md:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Media("(width >= 48rem)".to_lean_str()))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("aria-[busy=true]:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&[aria-busy=\"true\"]".to_lean_str()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("[&>svg]:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate("&>svg".to_lean_str()))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("motion-reduce:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::Media(
            "(prefers-reduced-motion: reduce)".to_lean_str()
        ))
    );
}

#[test]
fn resolves_dynamic_media_structural_and_attribute_variants() {
    let theme = theme();
    let resolve = |candidate: &str| {
        let candidate = parse_candidate(candidate).unwrap();
        resolve_variant(&candidate.variants[0], &theme)
    };

    assert_eq!(
        resolve("min-[40rem]:flex"),
        Some(VariantApplication::Media("(width >= 40rem)".to_lean_str()))
    );
    assert_eq!(
        resolve("@sm:flex"),
        Some(VariantApplication::Container(
            "(width >= 24rem)".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("nth-[3n+1]:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&:nth-child(3n+1)".to_lean_str()
        ))
    );
    assert_eq!(resolve("nth-[not-an-nth]:flex"), None);
    assert_eq!(
        resolve("in-focus:flex"),
        Some(VariantApplication::SelectorTemplate(
            ":where(:focus) &".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("has-checked:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&:has(:checked)".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("supports-[display:grid]:flex"),
        Some(VariantApplication::Supports("display:grid".to_lean_str()))
    );
    assert_eq!(
        resolve("aria-checked:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&[aria-checked=\"true\"]".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("data-[state=open]:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&[data-state=\"open\"]".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("in-range:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&:in-range".to_lean_str()
        ))
    );
    assert_eq!(
        resolve("not-focus:flex"),
        Some(VariantApplication::SelectorTemplate(
            "&:not(:focus)".to_lean_str()
        ))
    );
}

#[test]
fn reports_variant_selectors_that_do_not_depend_on_the_subject() {
    let theme = theme();
    let document_scoped = |candidate: &str| {
        let candidate = parse_candidate(candidate).unwrap();
        resolve_variant(&candidate.variants[0], &theme)
            .and_then(|application| document_scoped_selector(&application).map(LeanString::from))
    };

    assert_eq!(document_scoped("[:host]:flex").as_deref(), Some(":host"));
    assert_eq!(
        document_scoped("[::view-transition-old(*)]:flex").as_deref(),
        Some("::view-transition-old(*)")
    );
    assert_eq!(document_scoped("[&:hover]:flex"), None);
    assert_eq!(document_scoped("hover:flex"), None);
    assert_eq!(document_scoped("motion-reduce:flex"), None);
    assert_eq!(document_scoped("supports-[display:grid]:flex"), None);
}
