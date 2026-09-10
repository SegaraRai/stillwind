pub(crate) fn strip_framework_imports(css: &str) -> String {
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
                "stillwind"
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

/// Extracts only Stillwind's compatible Tailwind entry points.  This is kept
/// independent from fixtures: it understands ordinary CSS import syntax and
/// ignores all non-framework imports for the browser to resolve.
pub(crate) fn framework_imports(css: &str) -> FrameworkImports {
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
            "stillwind" => {
                found = true;
                output.layer_order = true;
                output.theme_layer = Some(layer.clone().unwrap_or_else(|| "theme".to_owned()));
                output.base_layer = Some(layer.clone().unwrap_or_else(|| "base".to_owned()));
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

pub(crate) fn parse_import_path(rule: &str) -> Option<(&str, &str)> {
    let quote = rule.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let end = rule[1..].find(quote)? + 1;
    Some((&rule[1..end], &rule[end + 1..]))
}

pub(crate) fn import_layer(tail: &str) -> Option<String> {
    let start = tail.find("layer(")? + "layer(".len();
    let end = tail[start..].find(')')? + start;
    let layer = tail[start..end].trim();
    (!layer.is_empty()).then(|| layer.to_owned())
}

pub(crate) fn import_prefix(tail: &str) -> Option<String> {
    let start = tail.find("prefix(")? + "prefix(".len();
    let end = tail[start..].find(')')? + start;
    let prefix = tail[start..end].trim();
    (!prefix.is_empty()).then(|| prefix.to_owned())
}

/// The Tailwind entry points selected by an authored `@import` directive.
/// Layer names are retained rather than inferred during rendering so a custom
/// `layer(...)` placement remains observable in the generated stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkImports {
    pub theme_layer: Option<String>,
    pub base_layer: Option<String>,
    pub utilities_layer: Option<String>,
    pub builtins: bool,
    /// Whether the canonical single full-framework import was authored.
    /// Tailwind emits the global layer-order prelude only for that entrypoint.
    pub layer_order: bool,
    /// Tailwind's class/theme namespace from `prefix(name)` import modifiers.
    pub prefix: Option<String>,
}

impl Default for FrameworkImports {
    fn default() -> Self {
        Self {
            theme_layer: Some("theme".to_owned()),
            base_layer: Some("base".to_owned()),
            utilities_layer: Some("utilities".to_owned()),
            builtins: true,
            layer_order: true,
            prefix: None,
        }
    }
}

impl FrameworkImports {
    pub(crate) fn explicit() -> Self {
        Self {
            theme_layer: None,
            base_layer: None,
            utilities_layer: None,
            builtins: false,
            layer_order: false,
            prefix: None,
        }
    }

    #[must_use]
    pub fn has_any(&self) -> bool {
        self.theme_layer.is_some() || self.base_layer.is_some() || self.utilities_layer.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_modifiers_and_framework_selection_are_independent() {
        assert_eq!(
            parse_import_path("'stillwind' layer(base)"),
            Some(("stillwind", " layer(base)"))
        );
        assert_eq!(parse_import_path("'unclosed"), None);
        assert_eq!(
            import_layer("layer( reset ) prefix(tw)"),
            Some("reset".into())
        );
        assert_eq!(import_layer("layer()"), None);
        assert_eq!(import_prefix("prefix(tw)"), Some("tw".into()));
        assert_eq!(import_prefix("prefix("), None);
        let imports = framework_imports("@import 'stillwind/preflight.css' layer(reset);");
        assert!(imports.has_any());
        assert_eq!(imports.base_layer.as_deref(), Some("reset"));
        assert_eq!(imports.theme_layer, None);
        assert_eq!(
            strip_framework_imports("@import 'stillwind';@import 'local.css';"),
            "@import 'local.css';"
        );
        assert_eq!(
            strip_framework_imports("@import 'unfinished'"),
            "@import 'unfinished'"
        );
    }
}
