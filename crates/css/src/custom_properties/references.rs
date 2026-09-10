use std::collections::BTreeSet;

pub fn custom_property_references(css: &str) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("--") {
        let start = cursor + relative;
        let end = css[start..]
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .last()
            .map_or(start + 2, |(offset, character)| {
                start + offset + character.len_utf8()
            });
        if end > start + 2 {
            references.insert(css[start..end].to_owned());
        }
        cursor = end.max(start + 2);
    }
    references
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_names_and_stops_at_css_delimiters() {
        assert_eq!(
            custom_property_references("var(--x, --fallback_2) --x --"),
            BTreeSet::from(["--x".into(), "--fallback_2".into()])
        );
        assert!(custom_property_references("color:red").is_empty());
    }
}
