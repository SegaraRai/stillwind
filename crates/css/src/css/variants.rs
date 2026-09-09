use super::blocks::{extract_named_blocks, strip_css_comments};
use super::definitions::CustomVariant;
use super::registry::parse_variant_name;

pub(super) fn extract_custom_variants(css: &str) -> Vec<CustomVariant> {
    let css = strip_css_comments(css);
    let mut variants = css
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.contains('{') {
                return None;
            }
            let value = line.strip_prefix("@custom-variant ")?.trim_end_matches(';');
            let (name, selector) = value.split_once(' ')?;
            let (name, wildcard) = parse_variant_name(name)?;
            Some(CustomVariant {
                name,
                wildcard,
                selector: selector
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .to_owned(),
                body: String::new(),
                builtin_order: None,
                sort: None,
                value_type: None,
                modifier_type: None,
            })
        })
        .collect::<Vec<_>>();
    variants.extend(
        extract_named_blocks(&css, "@custom-variant")
            .into_iter()
            .filter_map(|(header, body)| {
                let (name, wildcard) = parse_variant_name(header.split_whitespace().next()?)?;
                Some(CustomVariant {
                    name,
                    wildcard,
                    selector: String::new(),
                    body,
                    builtin_order: None,
                    sort: None,
                    value_type: None,
                    modifier_type: None,
                })
            }),
    );
    variants.extend(
        extract_named_blocks(&css, "@variant")
            .into_iter()
            .filter_map(|(header, body)| {
                if !body.contains("@slot") {
                    return None;
                }
                let (name, wildcard) = parse_variant_name(header.split_whitespace().next()?)?;
                Some(CustomVariant {
                    name,
                    wildcard,
                    selector: String::new(),
                    body,
                    builtin_order: None,
                    sort: None,
                    value_type: None,
                    modifier_type: None,
                })
            }),
    );
    variants
}
