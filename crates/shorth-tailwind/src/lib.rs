//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::fmt::Write;

use shorth_css::{ThemeIndex, ThemeNamespace};
use shorth_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    pub declaration: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    Media(String),
    Supports(String),
}

#[derive(Clone, Copy)]
struct StaticUtility {
    name: &'static str,
    declaration: &'static str,
}

#[derive(Clone, Copy)]
struct ThemeUtility {
    prefix: &'static str,
    namespace: ThemeNamespace,
    property: &'static str,
    supports_negative: bool,
}

const STATIC_UTILITIES: &[StaticUtility] = &[
    StaticUtility {
        name: "block",
        declaration: "display:block;",
    },
    StaticUtility {
        name: "inline-block",
        declaration: "display:inline-block;",
    },
    StaticUtility {
        name: "contents",
        declaration: "display:contents;",
    },
    StaticUtility {
        name: "flow-root",
        declaration: "display:flow-root;",
    },
    StaticUtility {
        name: "flex",
        declaration: "display:flex;",
    },
    StaticUtility {
        name: "inline-flex",
        declaration: "display:inline-flex;",
    },
    StaticUtility {
        name: "flex-row",
        declaration: "flex-direction:row;",
    },
    StaticUtility {
        name: "flex-col",
        declaration: "flex-direction:column;",
    },
    StaticUtility {
        name: "flex-wrap",
        declaration: "flex-wrap:wrap;",
    },
    StaticUtility {
        name: "flex-1",
        declaration: "flex:1 1 0%;",
    },
    StaticUtility {
        name: "grow",
        declaration: "flex-grow:1;",
    },
    StaticUtility {
        name: "grow-0",
        declaration: "flex-grow:0;",
    },
    StaticUtility {
        name: "shrink",
        declaration: "flex-shrink:1;",
    },
    StaticUtility {
        name: "shrink-0",
        declaration: "flex-shrink:0;",
    },
    StaticUtility {
        name: "grid",
        declaration: "display:grid;",
    },
    StaticUtility {
        name: "hidden",
        declaration: "display:none;",
    },
    StaticUtility {
        name: "visible",
        declaration: "visibility:visible;",
    },
    StaticUtility {
        name: "invisible",
        declaration: "visibility:hidden;",
    },
    StaticUtility {
        name: "place-items-center",
        declaration: "place-items:center;",
    },
    StaticUtility {
        name: "place-content-center",
        declaration: "place-content:center;",
    },
    StaticUtility {
        name: "items-center",
        declaration: "align-items:center;",
    },
    StaticUtility {
        name: "items-start",
        declaration: "align-items:flex-start;",
    },
    StaticUtility {
        name: "items-end",
        declaration: "align-items:flex-end;",
    },
    StaticUtility {
        name: "justify-center",
        declaration: "justify-content:center;",
    },
    StaticUtility {
        name: "justify-between",
        declaration: "justify-content:space-between;",
    },
    StaticUtility {
        name: "justify-start",
        declaration: "justify-content:flex-start;",
    },
    StaticUtility {
        name: "justify-end",
        declaration: "justify-content:flex-end;",
    },
    StaticUtility {
        name: "text-center",
        declaration: "text-align:center;",
    },
    StaticUtility {
        name: "text-left",
        declaration: "text-align:left;",
    },
    StaticUtility {
        name: "font-medium",
        declaration: "font-weight:500;",
    },
    StaticUtility {
        name: "font-semibold",
        declaration: "font-weight:600;",
    },
    StaticUtility {
        name: "font-bold",
        declaration: "font-weight:700;",
    },
    StaticUtility {
        name: "cursor-wait",
        declaration: "cursor:wait;",
    },
    StaticUtility {
        name: "cursor-pointer",
        declaration: "cursor:pointer;",
    },
    StaticUtility {
        name: "relative",
        declaration: "position:relative;",
    },
    StaticUtility {
        name: "absolute",
        declaration: "position:absolute;",
    },
    StaticUtility {
        name: "fixed",
        declaration: "position:fixed;",
    },
    StaticUtility {
        name: "sticky",
        declaration: "position:sticky;",
    },
    StaticUtility {
        name: "static",
        declaration: "position:static;",
    },
    StaticUtility {
        name: "border",
        declaration: "border-width:1px;",
    },
    StaticUtility {
        name: "border-0",
        declaration: "border-width:0;",
    },
    StaticUtility {
        name: "outline-0",
        declaration: "outline-width:0;",
    },
    StaticUtility {
        name: "outline-1",
        declaration: "outline-width:1px;",
    },
    StaticUtility {
        name: "outline-2",
        declaration: "outline-width:2px;",
    },
    StaticUtility {
        name: "outline-offset-2",
        declaration: "outline-offset:2px;",
    },
    StaticUtility {
        name: "position-area-block-end",
        declaration: "position-area:block-end;",
    },
    StaticUtility {
        name: "overflow-hidden",
        declaration: "overflow:hidden;",
    },
    StaticUtility {
        name: "opacity-0",
        declaration: "opacity:0;",
    },
    StaticUtility {
        name: "opacity-60",
        declaration: "opacity:.6;",
    },
    StaticUtility {
        name: "opacity-100",
        declaration: "opacity:1;",
    },
    StaticUtility {
        name: "translate-x-0",
        declaration: "transform:translateX(0);",
    },
    StaticUtility {
        name: "translate-y-0",
        declaration: "transform:translateY(0);",
    },
    StaticUtility {
        name: "transition",
        declaration: "transition-property:color,background-color,border-color,outline-color,text-decoration-color,fill,stroke,opacity,box-shadow,transform,filter,backdrop-filter;transition-duration:.15s;transition-timing-function:cubic-bezier(.4,0,.2,1);",
    },
];

const THEME_UTILITIES: &[ThemeUtility] = &[
    ThemeUtility {
        prefix: "bg-",
        namespace: ThemeNamespace::Color,
        property: "background-color",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "text-",
        namespace: ThemeNamespace::Color,
        property: "color",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "border-",
        namespace: ThemeNamespace::Color,
        property: "border-color",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "outline-",
        namespace: ThemeNamespace::Color,
        property: "outline-color",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "p-",
        namespace: ThemeNamespace::Spacing,
        property: "padding",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "px-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-inline",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "py-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-block",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "pt-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-top",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "pr-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-right",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "pb-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-bottom",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "pl-",
        namespace: ThemeNamespace::Spacing,
        property: "padding-left",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "m-",
        namespace: ThemeNamespace::Spacing,
        property: "margin",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "mx-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-inline",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "my-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-block",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "mt-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-top",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "mr-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-right",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "mb-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-bottom",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "ml-",
        namespace: ThemeNamespace::Spacing,
        property: "margin-left",
        supports_negative: true,
    },
    ThemeUtility {
        prefix: "gap-",
        namespace: ThemeNamespace::Spacing,
        property: "gap",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "w-",
        namespace: ThemeNamespace::Spacing,
        property: "width",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "h-",
        namespace: ThemeNamespace::Spacing,
        property: "height",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "min-w-",
        namespace: ThemeNamespace::Spacing,
        property: "min-width",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "min-h-",
        namespace: ThemeNamespace::Spacing,
        property: "min-height",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "max-w-",
        namespace: ThemeNamespace::Spacing,
        property: "max-width",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "max-h-",
        namespace: ThemeNamespace::Spacing,
        property: "max-height",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "basis-",
        namespace: ThemeNamespace::Spacing,
        property: "flex-basis",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "rounded-",
        namespace: ThemeNamespace::Radius,
        property: "border-radius",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "shadow-",
        namespace: ThemeNamespace::Shadow,
        property: "box-shadow",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "text-",
        namespace: ThemeNamespace::Text,
        property: "font-size",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "font-",
        namespace: ThemeNamespace::Font,
        property: "font-family",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "leading-",
        namespace: ThemeNamespace::Leading,
        property: "line-height",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "tracking-",
        namespace: ThemeNamespace::Tracking,
        property: "letter-spacing",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "z-",
        namespace: ThemeNamespace::ZIndex,
        property: "z-index",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "duration-",
        namespace: ThemeNamespace::Duration,
        property: "transition-duration",
        supports_negative: false,
    },
    ThemeUtility {
        prefix: "ease-",
        namespace: ThemeNamespace::Ease,
        property: "transition-timing-function",
        supports_negative: false,
    },
];

/// Resolves a candidate using only the Tailwind-compatible registry.
#[must_use]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    let declaration = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => format!("{property}:{value};"),
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    Some(ResolvedUtility {
        declaration: if candidate.important {
            important(&declaration)
        } else {
            declaration
        },
        candidate,
    })
}

#[must_use]
pub fn resolve_variant(variant: &Variant, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let VariantKind::Arbitrary(value) = &variant.kind {
        if let Some(condition) = value
            .strip_prefix("@supports(")
            .and_then(|value| value.strip_suffix(')'))
        {
            return Some(VariantApplication::Supports(condition.to_owned()));
        }
        return Some(VariantApplication::SelectorTemplate(value.clone()));
    }
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    let suffix = match value.as_str() {
        "hover" => Some(":hover"),
        "focus" => Some(":focus"),
        "focus-visible" => Some(":focus-visible"),
        "active" => Some(":active"),
        "disabled" => Some(":disabled"),
        "before" => Some("::before"),
        "after" => Some("::after"),
        _ => None,
    };
    if let Some(suffix) = suffix {
        return Some(VariantApplication::SelectorSuffix(suffix));
    }
    if value == "dark" {
        return Some(VariantApplication::Media(
            "(prefers-color-scheme: dark)".to_owned(),
        ));
    }
    if let Some(condition) = value
        .strip_prefix("supports-[")
        .and_then(|value| value.strip_suffix(']'))
    {
        return Some(VariantApplication::Supports(condition.replace('_', " ")));
    }
    for (prefix, attribute) in [("aria-[", "aria"), ("data-[", "data")] {
        if let Some(value) = value
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(']'))
            && let Some((name, expected)) = value.split_once('=')
        {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&[{attribute}-{name}=\"{expected}\"]"
            )));
        }
    }
    if let Some(custom) = theme.variant(value) {
        return Some(VariantApplication::SelectorTemplate(
            custom.selector.clone(),
        ));
    }
    theme
        .token_in(ThemeNamespace::Breakpoint, value)
        .map(|token| VariantApplication::Media(format!("(width >= {})", token.value.value)))
}

fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    if let Some(custom) = theme.utility(name)
        && !custom.body.contains('&')
    {
        return (!negative && modifier.is_none()).then(|| custom.body.clone());
    }
    if let Some(declaration) = STATIC_UTILITIES
        .iter()
        .find(|utility| utility.name == name)
        .map(|utility| utility.declaration.to_owned())
    {
        return (!negative && modifier.is_none()).then_some(declaration);
    }
    if let Some(columns) = name.strip_prefix("grid-cols-")
        && let Ok(columns) = columns.parse::<u8>()
        && (1..=12).contains(&columns)
        && !negative
        && modifier.is_none()
    {
        return Some(format!(
            "grid-template-columns:repeat({columns},minmax(0,1fr));"
        ));
    }
    resolve_arbitrary(name, negative)
        .or_else(|| resolve_special(name, modifier, negative, theme))
        .or_else(|| resolve_theme_utility(name, modifier, negative, theme))
}

fn resolve_special(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    if let Some(value) = name.strip_prefix("animate-")
        && theme.token_in(ThemeNamespace::Animate, value).is_some()
        && !negative
        && modifier.is_none()
    {
        return Some(format!("animation:var(--animate-{value});"));
    }
    if let Some(value) = name.strip_prefix("size-") {
        let value = theme_value(ThemeNamespace::Spacing, value, negative, theme)?;
        return Some(format!("width:{value};height:{value};"));
    }
    for (prefix, property) in [
        ("translate-x-", "translateX"),
        ("translate-y-", "translateY"),
    ] {
        if let Some(value) = name.strip_prefix(prefix) {
            let value = theme_value(ThemeNamespace::Spacing, value, negative, theme)?;
            return Some(format!("transform:{property}({value});"));
        }
    }
    if let Some(percent) = name
        .strip_prefix("scale-")
        .and_then(|value| value.parse::<u16>().ok())
        && !negative
        && modifier.is_none()
    {
        return Some(format!("transform:scale({});", f64::from(percent) / 100.0));
    }
    None
}

fn resolve_theme_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    for utility in THEME_UTILITIES {
        let Some(value) = name.strip_prefix(utility.prefix) else {
            continue;
        };
        if negative && !utility.supports_negative {
            continue;
        }
        if utility.namespace == ThemeNamespace::Color {
            let Some(token) = theme.token_in(ThemeNamespace::Color, value) else {
                continue;
            };
            let value = match modifier {
                Some(modifier) => color_mix(&token.name.value, &opacity(modifier)?),
                None => format!("var({})", token.name.value),
            };
            return Some(format!("{}:{value};", utility.property));
        }
        if modifier.is_none() {
            let value = theme_value(utility.namespace, value, negative, theme)?;
            return Some(format!("{}:{value};", utility.property));
        }
    }
    None
}

fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = if let Some(value) = arbitrary(value) {
        value
    } else if let Some(value) = css_variable(value) {
        format!("var({value})")
    } else if theme.token_in(namespace, value).is_some() {
        format!("var(--{}-{value})", namespace.css_name())
    } else if namespace == ThemeNamespace::Spacing && theme.scale(ThemeNamespace::Spacing).is_some()
    {
        if value == "px" {
            "1px".to_owned()
        } else if value.parse::<f64>().is_ok() {
            let multiplier = if negative {
                format!("-{value}")
            } else {
                value.to_owned()
            };
            return Some(format!("calc(var(--spacing) * {multiplier})"));
        } else {
            return None;
        }
    } else {
        return None;
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn resolve_arbitrary(name: &str, negative: bool) -> Option<String> {
    let mappings = [
        ("bg-", "background-color"),
        ("text-", "color"),
        ("border-", "border-color"),
        ("outline-", "outline-color"),
        ("p-", "padding"),
        ("px-", "padding-inline"),
        ("py-", "padding-block"),
        ("m-", "margin"),
        ("mx-", "margin-inline"),
        ("my-", "margin-block"),
        ("w-", "width"),
        ("h-", "height"),
        ("min-w-", "min-width"),
        ("min-h-", "min-height"),
        ("max-w-", "max-width"),
        ("max-h-", "max-height"),
        ("gap-", "gap"),
        ("rounded-", "border-radius"),
    ];
    for (prefix, property) in mappings {
        if let Some(value) = name.strip_prefix(prefix).and_then(arbitrary) {
            let value = if negative {
                format!("calc({value} * -1)")
            } else {
                value
            };
            return Some(format!("{property}:{value};"));
        }
    }
    None
}

fn arbitrary(value: &str) -> Option<String> {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map(decode_arbitrary)
}

fn css_variable(value: &str) -> Option<&str> {
    value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
}

fn decode_arbitrary(value: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '_' {
            result.push(' ');
        } else {
            result.push(character);
        }
    }
    result
}

fn opacity(value: &Value) -> Option<String> {
    let value = match value {
        Value::Named(value) | Value::Arbitrary(value) => value,
        Value::CssVariable(value) => return Some(format!("var({value})")),
    };
    let number = value.parse::<f64>().ok()?;
    Some(if number <= 1.0 {
        (number * 100.0).to_string()
    } else {
        number.to_string()
    })
}

fn color_mix(token: &str, opacity: &str) -> String {
    format!("color-mix(in oklab,var({token}) {opacity}%,transparent)")
}

fn important(declaration: &str) -> String {
    declaration
        .split_terminator(';')
        .filter(|declaration| !declaration.is_empty())
        .fold(String::new(), |mut result, declaration| {
            write!(result, "{declaration}!important;").expect("writing to a String cannot fail");
            result
        })
}

#[cfg(test)]
mod tests {
    use shorth_css::parse_tailwind_theme;
    use shorth_source::{FileId, SourceFile};

    use super::*;

    fn theme() -> ThemeIndex {
        parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-brand: red; }",
        ))
        .unwrap()
    }

    #[test]
    fn resolves_fixture_style_static_theme_and_arbitrary_candidates() {
        let theme = theme();
        assert_eq!(
            resolve_utility("!flex", &theme).unwrap().declaration,
            "display:flex!important;"
        );
        assert_eq!(
            resolve_utility("-mt-4", &theme).unwrap().declaration,
            "margin-top:calc(var(--spacing) * -4);"
        );
        assert_eq!(
            resolve_utility("bg-brand/50", &theme).unwrap().declaration,
            "background-color:color-mix(in oklab,var(--color-brand) 50%,transparent);"
        );
        assert_eq!(
            resolve_utility("w-(--sidebar)", &theme)
                .unwrap()
                .declaration,
            "width:var(--sidebar);"
        );
        assert_eq!(
            resolve_utility("[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color:rebeccapurple;"
        );
    }

    #[test]
    fn resolves_state_attribute_responsive_and_selector_variants() {
        let theme = theme();
        assert_eq!(
            resolve_variant(&parse_candidate("hover:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::SelectorSuffix(":hover"))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("md:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width >= 48rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("aria-[busy=true]:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-busy=\"true\"]".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("[&>svg]:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate("&>svg".to_owned()))
        );
    }
}
