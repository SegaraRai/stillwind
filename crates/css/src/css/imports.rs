use super::FrameworkImports;

/// Extracts only Stillwind's compatible Tailwind entry points.  This is kept
/// independent from fixtures: it understands ordinary CSS import syntax and
/// ignores all non-framework imports for the browser to resolve.
pub(super) fn framework_imports(css: &str) -> FrameworkImports {
    let imports = css
        .split('@')
        .filter_map(|segment| segment.strip_prefix("import "))
        .filter_map(|segment| segment.split_once(';').map(|(rule, _)| rule));
    let mut found = false;
    let mut output = FrameworkImports::explicit();

    for rule in imports {
        let rule = rule.trim();
        let Some((path, tail)) = parse_import_path(rule) else {
            continue;
        };
        let layer = import_layer(tail);
        let prefix = import_prefix(tail);
        match path {
            "tailwindcss" | "stillwind" => {
                found = true;
                output.layer_order = true;
                output.theme_layer = Some(layer.clone().unwrap_or_else(|| "theme".to_owned()));
                output.base_layer = Some(layer.clone().unwrap_or_else(|| "base".to_owned()));
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
                output.builtins = true;
                output.prefix.clone_from(&prefix);
            }
            "tailwindcss/theme" => {
                found = true;
                output.theme_layer = Some(layer.unwrap_or_else(|| "theme".to_owned()));
                output.builtins = true;
                output.prefix.clone_from(&prefix);
            }
            "tailwindcss/preflight" => {
                found = true;
                output.base_layer = Some(layer.unwrap_or_else(|| "base".to_owned()));
                output.builtins = true;
            }
            "tailwindcss/utilities" => {
                found = true;
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
                output.builtins = true;
                output.prefix.clone_from(&prefix);
            }
            "stillwind/theme.css" => {
                found = true;
                output.theme_layer = Some(layer.unwrap_or_else(|| "theme".to_owned()));
            }
            "stillwind/preflight.css" => {
                found = true;
                output.base_layer = Some(layer.unwrap_or_else(|| "base".to_owned()));
            }
            "stillwind/utilities.css" => {
                found = true;
                output.utilities_layer = Some(layer.unwrap_or_else(|| "utilities".to_owned()));
            }
            "stillwind/builtins.css" => {
                found = true;
                output.builtins = true;
            }
            _ => {}
        }
    }

    if found {
        output
    } else {
        FrameworkImports::default()
    }
}

pub(super) fn parse_import_path(rule: &str) -> Option<(&str, &str)> {
    let quote = rule.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let end = rule[1..].find(quote)? + 1;
    Some((&rule[1..end], &rule[end + 1..]))
}

pub(super) fn import_layer(tail: &str) -> Option<String> {
    let start = tail.find("layer(")? + "layer(".len();
    let end = tail[start..].find(')')? + start;
    let layer = tail[start..end].trim();
    (!layer.is_empty()).then(|| layer.to_owned())
}

pub(super) fn import_prefix(tail: &str) -> Option<String> {
    let start = tail.find("prefix(")? + "prefix(".len();
    let end = tail[start..].find(')')? + start;
    let prefix = tail[start..end].trim();
    (!prefix.is_empty()).then(|| prefix.to_owned())
}
