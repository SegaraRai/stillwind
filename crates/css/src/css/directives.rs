use super::{
    BTreeSet, CustomPropertyActivation, CustomPropertyDefinition, StillwindConfig,
    extract_named_blocks, parse_import_path, strip_css_comments,
};

pub(super) fn strip_framework_directives(css: &str) -> String {
    let css = strip_framework_imports(css);
    let css = strip_blocks(&css, "@theme");
    let css = strip_blocks(&css, "@utility");
    let css = strip_top_level_blocks(&css, "@variant");
    let css = strip_blocks(&css, "@custom-variant");
    let css = strip_blocks(&css, "@stillwind-property");
    let css = strip_blocks(&css, "@stillwind");
    css.lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("@custom-variant") && !line.starts_with("@tailwind")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn extract_stillwind_config(css: &str) -> StillwindConfig {
    let mut config = StillwindConfig::default();
    for (_, body) in extract_named_blocks(&strip_css_comments(css), "@stillwind") {
        for declaration in body.split(';') {
            let declaration = declaration.trim();
            let Some((key, value)) = declaration.split_once(':') else {
                continue;
            };
            let key = key
                .rsplit_once('{')
                .map_or(key.trim(), |(_, nested)| nested.trim());
            let value = value.trim().trim_matches(['"', '\'']);
            if key == "prefix" {
                config.prefix = Some(value.to_owned());
            } else if key == "tailwind-prefix" {
                config.tailwind_prefix = Some(value.to_owned());
            } else if matches!(value, "warn" | "deny" | "off") {
                config.lint_rules.insert(key.to_owned(), value.to_owned());
            }
        }
    }
    config
}

pub(super) fn strip_framework_imports(css: &str) -> String {
    let mut output = String::with_capacity(css.len());
    let mut cursor = 0;

    while let Some(relative_start) = css[cursor..].find("@import") {
        let start = cursor + relative_start;
        output.push_str(&css[cursor..start]);
        let Some(relative_end) = css[start..].find(';') else {
            output.push_str(&css[start..]);
            return output;
        };
        let end = start + relative_end + 1;
        let rule = css[start + "@import".len()..end - 1].trim();
        let is_framework = parse_import_path(rule).is_some_and(|(path, _)| {
            matches!(
                path,
                "tailwindcss"
                    | "tailwindcss/theme"
                    | "tailwindcss/preflight"
                    | "tailwindcss/utilities"
                    | "stillwind"
                    | "stillwind/theme.css"
                    | "stillwind/preflight.css"
                    | "stillwind/utilities.css"
                    | "stillwind/builtins.css"
            )
        });
        if !is_framework {
            output.push_str(&css[start..end]);
        }
        cursor = end;
    }

    output.push_str(&css[cursor..]);
    output
}

pub(super) fn extract_property_definitions(css: &str) -> Vec<CustomPropertyDefinition> {
    extract_named_blocks(&strip_css_comments(css), "@stillwind-property")
        .into_iter()
        .filter_map(|(header, body)| {
            let name = header.split_whitespace().next()?;
            if !name.starts_with("--") {
                return None;
            }
            let mut activation = CustomPropertyActivation::default();
            let mut dependencies = Vec::new();
            let mut groups = Vec::new();
            let mut activation_properties = Vec::new();
            let mut standard_descriptors = Vec::new();
            for descriptor in body.split(';') {
                let trimmed = descriptor.trim();
                if let Some(value) = trimmed.strip_prefix("activation:") {
                    if let Some(parsed) = CustomPropertyActivation::parse(value) {
                        activation = parsed;
                    }
                } else if let Some(value) = trimmed.strip_prefix("depends-on:") {
                    dependencies.extend(value.split_whitespace().map(str::to_owned));
                } else if let Some(value) = trimmed.strip_prefix("group:") {
                    groups.extend(value.split_whitespace().map(str::to_owned));
                } else if let Some(value) = trimmed.strip_prefix("activate-on:") {
                    activation_properties.extend(value.split_whitespace().map(str::to_owned));
                } else if !trimmed.is_empty() {
                    standard_descriptors.push(format!("{trimmed};"));
                }
            }
            Some(CustomPropertyDefinition {
                name: name.to_owned(),
                body: standard_descriptors.join("\n"),
                activation,
                dependencies,
                groups,
                activation_properties,
            })
        })
        .collect()
}

pub(super) fn extract_authored_property_names(css: &str) -> BTreeSet<String> {
    extract_named_blocks(&strip_css_comments(css), "@property")
        .into_iter()
        .filter_map(|(header, _)| {
            let name = header.split_whitespace().next()?;
            name.starts_with("--").then(|| name.to_owned())
        })
        .collect()
}

pub(super) fn strip_blocks(css: &str, directive: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        output.push_str(&css[cursor..start]);
        let Some(open_relative) = find_block_open(&css[start..]) else {
            output.push_str(&css[start..]);
            break;
        };
        let open = start + open_relative;
        let mut depth = 1_u32;
        let mut close = open + 1;
        let body = &css[open + 1..];
        let mut offset = 0;
        while offset < body.len() {
            if body[offset..].starts_with("{modifier}") {
                offset += "{modifier}".len();
                continue;
            }
            let character = body[offset..].chars().next().unwrap();
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
            offset += character.len_utf8();
        }
        if depth != 0 {
            // Preserve malformed input for the parser instead of advancing past EOF.
            output.push_str(&css[start..]);
            return output;
        }
        cursor = close.saturating_add(1);
    }
    if cursor < css.len() {
        output.push_str(&css[cursor..]);
    }
    output
}

pub(super) fn find_block_open(value: &str) -> Option<usize> {
    let mut offset = 0;
    while offset < value.len() {
        if value[offset..].starts_with("{modifier}") {
            offset += "{modifier}".len();
            continue;
        }
        let character = value[offset..].chars().next()?;
        if character == '{' {
            return Some(offset);
        }
        offset += character.len_utf8();
    }
    None
}

pub(super) fn strip_top_level_blocks(css: &str, directive: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let mut depth = 0_u32;
    while cursor < css.len() {
        if depth == 0 && css[cursor..].starts_with(directive) {
            let Some(open_rel) = css[cursor..].find('{') else {
                break;
            };
            let open = cursor + open_rel;
            let mut inner = 1_u32;
            let mut end = open + 1;
            for (offset, ch) in css[open + 1..].char_indices() {
                match ch {
                    '{' => inner += 1,
                    '}' => {
                        inner = inner.saturating_sub(1);
                        if inner == 0 {
                            end = open + 1 + offset + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            cursor = end;
            continue;
        }
        let Some(ch) = css[cursor..].chars().next() else {
            break;
        };
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth = depth.saturating_sub(1);
        }
        output.push(ch);
        cursor += ch.len_utf8();
    }
    output.push_str(&css[cursor..]);
    output
}
