//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{collections::HashMap, fmt::Write, sync::OnceLock};

use stillwind_css::{
    CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace, ValueDomain, ValueType,
    color_with_alpha_fallback, shadow_with_color_fallback,
};
use stillwind_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    pub candidate: Candidate,
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
    /// Additional selector targets produced by typed `@utility` overloads.
    /// CSS fallbacks and selector-bearing branches must never be discarded
    /// merely because they share one candidate spelling.
    pub additional: Vec<ResolvedUtilityTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtilityTarget {
    pub declaration: String,
    pub selector: Option<String>,
    pub nested: Vec<NestedUtilityRule>,
}

/// A conditional declaration authored inside a CSS `@utility` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedUtilityRule {
    pub at_rule: NestedUtilityAtRule,
    pub condition: String,
    pub declaration: String,
}

/// The CSS conditional wrapper associated with a nested utility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedUtilityAtRule {
    Media,
    Supports,
}

type ParsedNestedUtilityRule = (NestedUtilityAtRule, String, String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct UtilityResolution {
    declaration: String,
    selector: Option<String>,
    nested: Vec<NestedUtilityRule>,
    additional: Vec<ResolvedUtilityTarget>,
}

impl UtilityResolution {
    fn plain(declaration: String) -> Self {
        Self {
            declaration,
            selector: None,
            nested: Vec::new(),
            additional: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    SelectorSuffix(&'static str),
    SelectorTemplate(String),
    SelectorMedia { selector: String, condition: String },
    Media(String),
    Container(String),
    Supports(String),
    StartingStyle,
}

const BUILTIN_REGISTRY_CSS: &str = concat!(
    include_str!("../../../packages/react/css/builtins.css"),
    include_str!("../../../packages/react/css/core-utilities.css"),
    include_str!("../../../packages/react/css/discrete-utilities.css"),
    include_str!("../../../packages/react/css/functional-utilities.css"),
    include_str!("../../../packages/react/css/interaction-variants.css"),
    include_str!("../../../packages/react/css/static-utilities.css"),
    include_str!("../../../packages/react/css/arbitrary-utilities.css"),
    include_str!("../../../packages/react/css/mask-utilities.css"),
    include_str!("../../../packages/react/css/effect-utilities.css"),
    include_str!("../../../packages/react/css/selector-variants.css"),
    include_str!("../../../packages/react/css/properties.css"),
);

struct BuiltinUtilityRegistry {
    builtin_exact: HashMap<String, Vec<CustomUtility>>,
    wildcards: Vec<CustomUtility>,
}

fn builtin_utilities() -> &'static BuiltinUtilityRegistry {
    static UTILITIES: OnceLock<BuiltinUtilityRegistry> = OnceLock::new();
    UTILITIES.get_or_init(|| {
        let normalize = |utility: CustomUtility| {
            let mut utility = utility;
            // The registry asset is intentionally formatted for humans. The generated
            // output must nevertheless be byte-stable across formatter versions.
            utility.body = canonical_builtin_body(&utility.body);
            utility
        };
        let builtins = stillwind_css::parse_utility_definitions(BUILTIN_REGISTRY_CSS)
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let mut builtin_exact = HashMap::<String, Vec<CustomUtility>>::new();
        let mut wildcards = Vec::new();
        for utility in builtins {
            if utility.name.ends_with('*') {
                wildcards.push(utility);
            } else {
                builtin_exact
                    .entry(utility.name.clone())
                    .or_default()
                    .push(utility);
            }
        }
        BuiltinUtilityRegistry {
            builtin_exact,
            wildcards,
        }
    })
}

fn builtin_variants() -> &'static [CustomVariant] {
    static VARIANTS: OnceLock<Vec<CustomVariant>> = OnceLock::new();
    VARIANTS
        .get_or_init(|| stillwind_css::parse_variant_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

/// Compiler-owned property definitions exposed by the built-in CSS entrypoint.
#[must_use]
pub fn builtin_property_definitions() -> &'static [stillwind_css::CustomPropertyDefinition] {
    static PROPERTIES: OnceLock<Vec<stillwind_css::CustomPropertyDefinition>> = OnceLock::new();
    PROPERTIES
        .get_or_init(|| stillwind_css::parse_property_definitions(BUILTIN_REGISTRY_CSS))
        .as_slice()
}

/// Removes non-semantic whitespace from vendored declarations.
///
/// This applies only to Stillwind-maintained CSS; user `@utility` bodies keep
/// their authored serialization. Quoted values are preserved byte-for-byte.
#[allow(clippy::too_many_lines)]
fn canonical_builtin_body(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut quote = None;
    let mut escaped = false;
    let mut whitespace = false;
    let mut functions = Vec::<String>::new();
    let mut gradient_declaration = false;
    let mut shadow_declaration = false;
    let mut mask_declaration = false;

    for character in value.chars() {
        if let Some(current_quote) = quote {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }

        match character {
            '\'' | '"' => {
                if whitespace && output.chars().last().is_some_and(is_word_character) {
                    output.push(' ');
                }
                whitespace = false;
                quote = Some(character);
                output.push(character);
            }
            ';' => {
                whitespace = false;
                gradient_declaration = false;
                shadow_declaration = false;
                mask_declaration = false;
                functions.clear();
                output.push(character);
            }
            '(' => {
                let name = output
                    .chars()
                    .rev()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                    })
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                functions.push(name);
                output.push(character);
            }
            ')' => {
                functions.pop();
                output.push(character);
            }
            ',' => {
                output.push(character);
                let preserve_function_spacing = functions
                    .iter()
                    .any(|name| matches!(name.as_str(), "color-mix" | "linear-gradient"));
                if gradient_declaration
                    || shadow_declaration
                    || mask_declaration
                    || preserve_function_spacing
                {
                    output.push(' ');
                }
                whitespace = false;
            }
            ':' if output.rsplit_once(';').is_some_and(|(_, declaration)| {
                declaration.ends_with("--tw-gradient-stops")
                    || declaration.ends_with("--tw-gradient-via-stops")
            }) =>
            {
                gradient_declaration = true;
                output.push(character);
            }
            ':' if output
                .rsplit_once(';')
                .is_some_and(|(_, declaration)| declaration.ends_with("box-shadow")) =>
            {
                shadow_declaration = true;
                output.push(character);
            }
            ':' if output.rsplit_once(';').is_some_and(|(_, declaration)| {
                declaration.starts_with("mask-image")
                    || declaration.ends_with("--tw-mask-linear-stops")
            }) =>
            {
                mask_declaration = true;
                output.push(character);
            }
            character if character.is_whitespace() => whitespace = true,
            character => {
                if whitespace && let Some(previous) = output.chars().last() {
                    let adjacent_words =
                        is_word_character(previous) && is_word_character(character);
                    let function_followed_by_word = previous == ')' && is_word_character(character);
                    // Keep a separator after commas. Tailwind's vendored
                    // utility serializer retains this spelling in function
                    // arguments (notably color-mix/gradient stop lists), and
                    // LightningCSS does not normalize it away in its output.
                    let preserve_function_spacing = functions
                        .iter()
                        .any(|name| matches!(name.as_str(), "color-mix" | "linear-gradient"));
                    if adjacent_words
                        || function_followed_by_word
                        || (previous == ',' && (gradient_declaration || preserve_function_spacing))
                    {
                        output.push(' ');
                    }
                }
                whitespace = false;
                output.push(character);
            }
        }
    }

    output
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '%')
}

fn normalize_conic_angles(mut declaration: &str) -> String {
    let mut output = String::with_capacity(declaration.len());
    while let Some(start) = declaration.find("calc(1deg*") {
        output.push_str(&declaration[..start]);
        let rest = &declaration[start + 10..];
        let (negative, digits) = if let Some(rest) = rest.strip_prefix("-1*") {
            (true, rest)
        } else {
            (false, rest)
        };
        let Some(end) = digits.find(')') else {
            output.push_str(&declaration[start..]);
            return output;
        };
        let amount = &digits[..end];
        if amount.is_empty() || !amount.chars().all(|character| character.is_ascii_digit()) {
            output.push_str(&declaration[start..start + 10]);
            declaration = rest;
            continue;
        }
        if negative {
            output.push_str(&format!("calc({amount}deg * -1)"));
        } else {
            output.push_str(&declaration[start..start + 10 + end + 1]);
        }
        declaration = &digits[end + 1..];
    }
    output.push_str(declaration);
    output
}

fn normalize_spaced_negative_angles(value: &str) -> String {
    let mut output = value.to_owned();
    while let Some(end) = output.find(" deg*-1)") {
        let number_end = end;
        let Some(space) = output[..number_end].rfind(' ') else {
            break;
        };
        let number_start = space + 1;
        let calc_start = output[..number_start].rfind("calc(");
        let Some(calc_start) = calc_start else { break };
        let number = &output[number_start..number_end];
        let replacement = format!("calc({number}deg * -1)");
        output.replace_range(calc_start..end + " deg*-1)".len(), &replacement);
    }
    output
}

fn normalize_negative_space_order(mut declaration: String, axis: &str) -> String {
    let (start_property, end_property) = if axis == "x" {
        ("margin-inline-start:", "margin-inline-end:")
    } else {
        ("margin-block-start:", "margin-block-end:")
    };
    let reverse = format!("--tw-space-{axis}-reverse");
    let (Some(start), Some(end)) = (
        declaration.find(start_property),
        declaration.find(end_property),
    ) else {
        return declaration;
    };
    let start_value = start + start_property.len();
    let end_value = end + end_property.len();
    let (Some(start_end), Some(end_end)) = (
        declaration[start_value..].find(';'),
        declaration[end_value..].find(';'),
    ) else {
        return declaration;
    };
    let first = declaration[start_value..start_value + start_end].to_owned();
    let second = declaration[end_value..end_value + end_end].to_owned();
    if first.contains(&format!("calc(1 - var({reverse}))"))
        && second.contains(&format!("var({reverse})"))
    {
        declaration.replace_range(start_value..start_value + start_end, &second);
        let adjusted_end =
            (end_value as isize + second.len() as isize - first.len() as isize) as usize;
        declaration.replace_range(adjusted_end..adjusted_end + end_end, &first);
    }
    declaration
}

fn normalize_positive_space_order(mut declaration: String, axis: &str) -> String {
    let (start_property, end_property) = if axis == "x" {
        ("margin-inline-start:", "margin-inline-end:")
    } else {
        ("margin-block-start:", "margin-block-end:")
    };
    let reverse = format!("--tw-space-{axis}-reverse");
    let (Some(start), Some(end)) = (
        declaration.find(start_property),
        declaration.find(end_property),
    ) else {
        return declaration;
    };
    let start_value = start + start_property.len();
    let end_value = end + end_property.len();
    let (Some(start_end), Some(end_end)) = (
        declaration[start_value..].find(';'),
        declaration[end_value..].find(';'),
    ) else {
        return declaration;
    };
    let first = declaration[start_value..start_value + start_end].to_owned();
    let second = declaration[end_value..end_value + end_end].to_owned();
    if first.contains(&format!("calc(1 - var({reverse}))"))
        && second.contains(&format!("var({reverse})"))
    {
        declaration.replace_range(start_value..start_value + start_end, &second);
        let adjusted_end =
            (end_value as isize + second.len() as isize - first.len() as isize) as usize;
        declaration.replace_range(adjusted_end..adjusted_end + end_end, &first);
    }
    declaration
}

/// Resolves a candidate using only the Tailwind-compatible registry.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn resolve_utility(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
    let candidate = parse_candidate(raw)?;
    if let Utility::Named {
        name,
        modifier: Some(modifier),
    } = &candidate.utility
        && matches!(modifier, Value::Named(value) if value.parse::<f64>().is_ok())
        && matches!(
            name.as_str(),
            "border"
                | "border-x"
                | "border-y"
                | "border-s"
                | "border-e"
                | "border-bs"
                | "border-be"
                | "border-t"
                | "border-r"
                | "border-b"
                | "border-l"
        )
    {
        return None;
    }
    if let Utility::Named {
        name,
        modifier: Some(Value::Named(value)),
    } = &candidate.utility
        && name == "text-shadow-inherit"
        && value.parse::<f64>().is_ok()
    {
        return None;
    }
    if let Utility::Named {
        name,
        modifier: Some(Value::Named(value)),
    } = &candidate.utility
        && name == "shadow-inherit"
        && value.parse::<f64>().is_ok()
    {
        return None;
    }
    let mut resolution = match &candidate.utility {
        Utility::ArbitraryProperty { property, value } => {
            UtilityResolution::plain(format!("{property}:{value};"))
        }
        Utility::Named { name, modifier } if name.starts_with("text-(") && name.ends_with(')') => {
            let body = &name[6..name.len() - 1];
            let (hint, value) = body.split_once(':')?;
            if hint != "length" || !value.starts_with("--") {
                return None;
            }
            UtilityResolution::plain(format!("font-size:var({value});"))
        }
        Utility::Named { name, modifier } => {
            resolve_named_utility(name, modifier.as_ref(), candidate.negative, theme)?
        }
    };
    if raw.starts_with("bg-linear-")
        && ["/longer", "/shorter", "/increasing", "/decreasing"]
        .iter()
        .any(|suffix| raw.ends_with(suffix))
        && let Some(value) = resolution
            .declaration
            .strip_prefix("--tw-gradient-position:")
            .and_then(|value| value.strip_suffix(';'))
        && let Some(split) = value.find(" in oklch")
    {
        let fallback = &value[..split];
        let modern = value.to_owned();
        resolution.declaration = resolution.declaration.replacen(value, fallback, 1);
        resolution.nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "background-image:linear-gradient(in lab, red, red)".to_owned(),
            declaration: format!("--tw-gradient-position:{modern};"),
        });
        for nested in &mut resolution.nested {
            if nested.at_rule == NestedUtilityAtRule::Supports {
                if let Some(split) = nested.declaration.find(";background-image:") {
                    nested.declaration.truncate(split + 1);
                }
            }
        }
        if !resolution.declaration.contains("background-image:") {
            let function = if raw.starts_with("bg-radial") {
                "radial-gradient"
            } else {
                "linear-gradient"
            };
            resolution.declaration.push_str(&format!(
                "background-image:{function}(var(--tw-gradient-stops));"
            ));
        }
    }
    if raw.contains("transparent/")
        && !raw.ends_with("/100")
        && (resolution.declaration.contains(":#0000;")
            || resolution.declaration.contains(":transparent;")
            || resolution
                .declaration
                .contains("color-mix(in srgb, transparent"))
        && let Some(modern) = resolution
            .nested
            .iter()
            .find(|nested| nested.declaration.contains("oklab"))
            .map(|nested| nested.declaration.clone())
    {
        resolution.declaration = modern;
        resolution.nested.clear();
    }
    if raw.ends_with("/100") {
        if resolution.declaration.contains(":#0000;")
            || resolution.declaration.contains(":transparent;")
        {
            resolution.nested.clear();
        }
        if resolution
            .nested
            .iter()
            .any(|nested| nested.declaration.contains("transparent") || nested.declaration.contains("/0)"))
        {
            resolution.nested.clear();
        }
        if resolution.declaration.contains("color-mix(in srgb, currentcolor 100%") {
            resolution.declaration = resolution
                .declaration
                .replace("color-mix(in srgb, currentcolor 100%, transparent)", "currentColor");
            resolution.nested.clear();
        } else if resolution
            .declaration
            .contains("color-mix(in srgb, inherit 100%, transparent)")
        {
            resolution.declaration = resolution
                .declaration
                .replace("color-mix(in srgb, inherit 100%, transparent)", "inherit");
            resolution.nested.clear();
        } else if resolution.declaration.contains("color-mix(in oklab, inherit 100%, transparent)") {
            resolution.declaration = resolution
                .declaration
                .replace("color-mix(in oklab, inherit 100%, transparent)", "inherit");
            resolution.nested.clear();
        } else if resolution.declaration.contains("color-mix(in oklab,inherit 100%,transparent)") {
            resolution.declaration = resolution
                .declaration
                .replace("color-mix(in oklab,inherit 100%,transparent)", "inherit");
            resolution.nested.clear();
        } else if resolution.declaration.contains("color-mix(in srgb, inherit")
            && let Some(modern) = resolution
                .nested
                .iter()
                .find(|nested| nested.declaration.contains("color-mix(in oklab"))
                .map(|nested| nested.declaration.clone())
        {
            resolution.declaration = modern;
            resolution.nested.clear();
        }
    } else if !raw.starts_with("placeholder-")
        && resolution.declaration.contains("color-mix(in srgb, inherit")
        && let Some(modern) = resolution
            .nested
            .iter()
            .find(|nested| nested.declaration.contains("color-mix(in oklab"))
            .map(|nested| nested.declaration.clone())
    {
        resolution.declaration = modern;
        resolution.nested.clear();
    }
    let mut declaration = if candidate.important {
        important(&resolution.declaration)
    } else {
        resolution.declaration
    };
    if raw.ends_with("/100") {
        declaration = declaration
            .replace("color-mix(in oklab, inherit 100%, transparent)", "inherit")
            .replace("color-mix(in oklab,inherit 100%,transparent)", "inherit");
        if raw.contains("-current/100") {
            declaration = declaration
                .replace("color-mix(in oklab, currentcolor 100%, transparent)", "currentcolor")
                .replace("color-mix(in oklab,currentcolor 100%,transparent)", "currentcolor");
        }
    }
    if raw.contains("-current/") {
        let mut cursor = 0;
        while let Some(relative) = declaration[cursor..].find(":color-mix(in srgb, currentcolor") {
            let start = cursor + relative;
            let Some(end_relative) = declaration[start..].find(';') else {
                break;
            };
            declaration.replace_range(start..start + end_relative, ":currentColor");
            cursor = start + ":currentColor".len();
        }
    }
    if matches!(&candidate.utility, Utility::Named { name, .. } if name.starts_with("mask-linear-from-") || name.starts_with("mask-linear-to-"))
    {
        declaration = declaration
            .replace("calc(var(--spacing) * 0)", "0px")
            .replace(":0;", ":0px;");
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && name.starts_with("backdrop-")
        && let Some(value) = declaration
            .strip_prefix("backdrop-filter:")
            .and_then(|value| value.strip_suffix(';'))
        && !declaration.contains("var(--tw-backdrop-brightness")
    {
        let property = name
            .strip_prefix("backdrop-")
            .unwrap_or_default()
            .split("-[")
            .next()
            .unwrap_or_default();
        declaration = format!(
            "--tw-backdrop-{property}:{value};backdrop-filter:var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);"
        );
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && let Some(value) = declaration
            .strip_prefix("filter:")
            .and_then(|value| value.strip_suffix(';'))
        && !declaration.contains("var(--tw-brightness")
        && let Some(property) = [
            "blur",
            "brightness",
            "contrast",
            "grayscale",
            "hue-rotate",
            "invert",
            "opacity",
            "saturate",
            "sepia",
        ]
        .into_iter()
        .find(|property| {
            let Some(suffix) = name.strip_prefix(&format!("{property}-")) else {
                return false;
            };
            (!name.contains('[') && suffix.parse::<f64>().is_ok()) || name.ends_with("-[initial]")
        })
        && matches!(
            property,
            "blur"
                | "brightness"
                | "contrast"
                | "grayscale"
                | "hue-rotate"
                | "invert"
                | "opacity"
                | "saturate"
                | "sepia"
        )
    {
        declaration = format!(
            "--tw-{property}:{value};filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
        );
    }
    if declaration == "--tw-drop-shadow:;" {
        declaration = "--tw-drop-shadow: ;".to_owned();
    }
    if declaration == "grid-column:span initial/span initial;" {
        declaration = "grid-column:span initial / span initial;".to_owned();
    }
    if declaration == "grid-row:span initial/span initial;" {
        declaration = "grid-row:span initial / span initial;".to_owned();
    }
    if matches!(&candidate.utility, Utility::Named { name, .. } if name == "transition")
        && declaration.starts_with("transition-property:")
    {
        declaration = "transition-property:color,background-color,border-color,outline-color,text-decoration-color,fill,stroke,--tw-gradient-from,--tw-gradient-via,--tw-gradient-to,opacity,box-shadow,transform,translate,scale,rotate,filter,-webkit-backdrop-filter,backdrop-filter,display,content-visibility,overlay,pointer-events;transition-timing-function:var(--tw-ease,var(--default-transition-timing-function));transition-duration:var(--tw-duration,var(--default-transition-duration));".to_owned();
    }
    if raw == "ease-initial" {
        declaration = "--tw-ease:initial;".to_owned();
    } else if raw == "ease-linear" {
        declaration = declaration.replace("--tw-ease:var(--ease-linear);", "--tw-ease:linear;");
    }
    // Tailwind canonicalizes zero-angle conic values instead of retaining the
    // registry's unit multiplier expression.
    declaration = declaration
        .replace("calc(1 deg*-1*", "calc(1deg*-1*")
        .replace("calc(1 deg * -1 *", "calc(1deg*-1*")
        .replace("calc(1deg * -1 *", "calc(1deg*-1*");
    declaration = normalize_conic_angles(&declaration);
    declaration = declaration.replace("calc(0 deg*-1)", "calc(0deg * -1)");
    declaration = normalize_spaced_negative_angles(&declaration);
    if raw.starts_with("mask-linear-") || raw.starts_with("mask-conic-") {
        declaration = declaration.replace("calc(1deg*", "calc(1deg * ");
    }
    if raw.starts_with("mask-") {
        declaration = declaration
            .replace("var(--tw-mask-bottom-from-color,black)", "var(--tw-mask-bottom-from-color)")
            .replace("var(--tw-mask-bottom-from-color, black)", "var(--tw-mask-bottom-from-color)")
            .replace("var(--tw-mask-bottom-from-position,0%)", "var(--tw-mask-bottom-from-position)")
            .replace("var(--tw-mask-bottom-from-position, 0%)", "var(--tw-mask-bottom-from-position)")
            .replace("var(--tw-mask-bottom-to-color,transparent)", "var(--tw-mask-bottom-to-color)")
            .replace("var(--tw-mask-bottom-to-color, transparent)", "var(--tw-mask-bottom-to-color)")
            .replace("var(--tw-mask-bottom-to-position,100%)", "var(--tw-mask-bottom-to-position)")
            .replace("var(--tw-mask-bottom-to-position, 100%)", "var(--tw-mask-bottom-to-position)")
            .replace("var(--tw-mask-radial-shape,ellipse)", "var(--tw-mask-radial-shape)")
            .replace("var(--tw-mask-radial-size,farthest-corner)", "var(--tw-mask-radial-size)")
            .replace("var(--tw-mask-radial-position,center)", "var(--tw-mask-radial-position)")
            .replace("var(--tw-mask-radial-from-color,black)", "var(--tw-mask-radial-from-color)")
            .replace("var(--tw-mask-radial-from-position,0%)", "var(--tw-mask-radial-from-position)")
            .replace("var(--tw-mask-radial-to-color,transparent)", "var(--tw-mask-radial-to-color)")
            .replace("var(--tw-mask-radial-to-position,100%)", "var(--tw-mask-radial-to-position)")
            .replace("var(--tw-mask-radial-shape,ellipse)", "var(--tw-mask-radial-shape)")
            .replace("var(--tw-mask-radial-size,farthest-corner)", "var(--tw-mask-radial-size)")
            .replace("var(--tw-mask-radial-position,center)", "var(--tw-mask-radial-position)")
            .replace("var(--tw-mask-radial-from-color,black)", "var(--tw-mask-radial-from-color)")
            .replace("var(--tw-mask-radial-from-position,0%)", "var(--tw-mask-radial-from-position)")
            .replace("var(--tw-mask-radial-to-color,transparent)", "var(--tw-mask-radial-to-color)")
            .replace("var(--tw-mask-radial-to-position,100%)", "var(--tw-mask-radial-to-position)")
            .replace(",black)", ")")
            .replace(", 0%)", ")")
            .replace(",0%)", ")")
            .replace(", 100%)", ")")
            .replace(",100%)", ")")
            .replace("),var(--tw-mask", "), var(--tw-mask");
    }
    declaration = declaration
        .replace("--tw-scrollbar-thumb:--tw-scrollbar-thumb:", "--tw-scrollbar-thumb:")
        .replace("--tw-scrollbar-track:--tw-scrollbar-track:", "--tw-scrollbar-track:");
    declaration = declaration
        .replace("--tw-scrollbar-thumb:--tw-scrollbar-thumb:", "--tw-scrollbar-thumb:")
        .replace("--tw-scrollbar-track:--tw-scrollbar-track:", "--tw-scrollbar-track:");
    if raw.starts_with("inset-ring-") {
        for width in 0..=64 {
            declaration = declaration.replace(
                &format!("calc({width}*1px)"),
                &format!("{width}px"),
            );
        }
    }
    if raw.starts_with("bg-conic-") {
        declaration = declaration.replace("calc(1deg*0)", "0deg");
        for amount in 1..=1000 {
            declaration = declaration.replace(
                &format!("calc(1deg*{amount})"),
                &format!("{amount}deg"),
            );
        }
    }
    if raw.starts_with("-mask-linear-") || raw.starts_with("-mask-conic-") {
        for number in 2..=1000 {
            declaration = declaration.replace(
                &format!("calc({number}deg * -1)"),
                &format!("calc(1deg * -{number})"),
            );
        }
        declaration = declaration
            .replace("calc(1deg * -1)", "-1deg")
            .replace("calc(0deg * -1)", "0deg");
    }
    if raw.starts_with("-outline-offset-") {
        if let Some(number) = raw
            .strip_prefix("-outline-offset-")
            .and_then(|value| value.parse::<f64>().ok())
        {
            let value = if number.fract() == 0.0 {
                format!("{}px", number as i64)
            } else {
                format!("{number}px")
            };
            declaration = format!("outline-offset:calc({value} * -1);");
        }
        for number in 0..=1000 {
            declaration = declaration.replace(
                &format!("calc(var(--spacing) * -{number})"),
                &format!("calc({number}px * -1)"),
            );
        }
    }
    if raw.starts_with("-space-x-") || raw.starts_with("-space-y-") {
        if raw == "-space-x-0" {
            declaration = "--tw-space-x-reverse:0;margin-inline:0;".to_owned();
        } else if raw == "-space-y-0" {
            declaration = "--tw-space-y-reverse:0;margin-block:0;".to_owned();
        }
        declaration = declaration
            .replace(
                "* -1*calc(1 - var(--tw-space-x-reverse))",
                " * calc(1 - var(--tw-space-x-reverse))",
            )
            .replace(
                ")*-1*calc(1 - var(--tw-space-x-reverse))",
                ") * calc(1 - var(--tw-space-x-reverse))",
            )
            .replace(
                "* -1*var(--tw-space-x-reverse)",
                " * var(--tw-space-x-reverse)",
            )
            .replace(
                ")*-1*var(--tw-space-x-reverse)",
                ") * var(--tw-space-x-reverse)",
            )
            .replace(
                "* -1*calc(1 - var(--tw-space-y-reverse))",
                " * calc(1 - var(--tw-space-y-reverse))",
            )
            .replace(
                ")*-1*calc(1 - var(--tw-space-y-reverse))",
                ") * calc(1 - var(--tw-space-y-reverse))",
            )
            .replace(
                "* -1*var(--tw-space-y-reverse)",
                " * var(--tw-space-y-reverse)",
            )
            .replace(")*-1*", ") * ")
            .replace("calc(calc(1px * -1) * ", "calc(-1px * ");
        declaration = normalize_negative_space_order(
            declaration,
            if raw.starts_with("-space-x-") {
                "x"
            } else {
                "y"
            },
        );
    }
    if raw.starts_with("space-x-") || raw.starts_with("space-y-") {
        if raw == "space-x-0" {
            declaration = "--tw-space-x-reverse:0;margin-inline:0;".to_owned();
        } else if raw == "space-y-0" {
            declaration = "--tw-space-y-reverse:0;margin-block:0;".to_owned();
        }
        declaration = declaration
            .replace("calc(calc(var(--spacing) * 1)*", "calc(var(--spacing) * ")
            .replace("px*", "px *")
            .replace("*var(--tw-space", "* var(--tw-space")
            .replace("*calc(1 - var(--tw-space", "* calc(1 - var(--tw-space");
        declaration = normalize_positive_space_order(
            declaration,
            if raw.starts_with("space-x-") {
                "x"
            } else {
                "y"
            },
        );
    }
    if (raw.starts_with("from-current/")
        || raw.starts_with("via-current/")
        || raw.starts_with("to-current/"))
        && !raw.ends_with("/100")
        && let Some(property) = ["from", "via", "to"].into_iter().find(|property| {
            declaration.starts_with(&format!(
                "--tw-gradient-{property}:color-mix(in oklab, currentcolor"
            ))
        })
    {
        let prefix = format!("--tw-gradient-{property}:");
        let value = declaration.trim_start_matches(&prefix);
        if let Some((color, rest)) = value.split_once(';') {
            declaration = format!(
                "{prefix}currentcolor;@supports (color:color-mix(in lab, red, red)){{--tw-gradient-{property}:{color}}}{rest}"
            );
        }
    }
    if (raw.ends_with("-inherit/100") || raw.ends_with("-transparent/100"))
        && let Some(property) = ["from", "via", "to"]
            .into_iter()
            .find(|property| declaration.starts_with(&format!("--tw-gradient-{property}:")))
    {
        let prefix = format!("--tw-gradient-{property}:");
        let value = if raw.ends_with("-inherit/100") {
            "inherit"
        } else {
            "transparent"
        };
        if let Some((_, rest)) = declaration.trim_start_matches(&prefix).split_once(';') {
            declaration = format!("{prefix}{value};{rest}");
        }
    }
    if raw == "translate-x-0"
        || raw == "translate-y-0"
        || raw == "translate-z-0"
        || raw == "translate-0"
    {
        declaration = declaration
            .replace("transform:translateX(0);", "")
            .replace("transform:translateY(0);", "")
            .replace("transform:translateZ(0);", "")
            .replace("transform:translate(0);", "");
    }
    if raw == "underline-offset-0" {
        declaration = declaration.replace("text-underline-offset:0;", "text-underline-offset:0px;");
    }
    if declaration.contains("--tw-translate-") {
        declaration = declaration
            .replace("calc(var(--spacing) * 0)", "0px")
            .replace("calc(var(--spacing) * 1)", "var(--spacing)");
    } else {
        declaration = declaration.replace("calc(var(--spacing) * 0)", "0");
    }
    declaration = declaration.replace("calc(var(--spacing) * 1)", "var(--spacing)");
    if raw.contains("translate-px") {
        declaration = declaration
            .replace("calc(var(--spacing) * -1)", "-1px")
            .replace("calc(var(--spacing)*-1)", "-1px");
    }
    if raw == "translate-0"
        || raw == "translate-z-0"
        || raw == "translate-x-0"
        || raw == "translate-y-0"
        || raw == "-translate-0"
        || raw == "-translate-z-0"
        || raw == "-translate-x-0"
        || raw == "-translate-y-0"
    {
        declaration = declaration
            .replace("--tw-translate-x:0;", "--tw-translate-x:0px;")
            .replace("--tw-translate-y:0;", "--tw-translate-y:0px;")
            .replace("--tw-translate-z:0;", "--tw-translate-z:0px;");
    }
    for number in 0..=1000 {
        declaration = declaration
            .replace(&format!("calc({number}*1%)"), &format!("{number}%"))
            .replace(&format!("calc({number}* -1%)"), &format!("-{number}%"))
            .replace(
                &format!("calc({number}*-1%)"),
                &format!("calc({number}% * -1)"),
            )
            .replace(
                &format!("skewX(calc({number}*1deg))"),
                &format!("skewX({number}deg)"),
            )
            .replace(
                &format!("skewX(calc({number}*-1deg))"),
                &format!("skewX(calc({number}deg * -1))"),
            )
            .replace(
                &format!("skewY(calc({number}*-1deg))"),
                &format!("skewY(calc({number}deg * -1))"),
            )
            .replace(
                &format!("rotateX(calc({number}*-1deg))"),
                &format!("rotateX(calc({number}deg * -1))"),
            )
            .replace(
                &format!("rotateY(calc({number}*-1deg))"),
                &format!("rotateY(calc({number}deg * -1))"),
            )
            .replace(
                &format!("rotateZ(calc({number}*-1deg))"),
                &format!("rotateZ(calc({number}deg * -1))"),
            );
    }
    declaration = declaration
        .replace("var(--tw-rotate-x)", "var(--tw-rotate-x,)")
        .replace("var(--tw-rotate-y)", "var(--tw-rotate-y,)")
        .replace("var(--tw-rotate-z)", "var(--tw-rotate-z,)")
        .replace("var(--tw-skew-x)", "var(--tw-skew-x,)")
        .replace("var(--tw-skew-y)", "var(--tw-skew-y,)");
    if declaration == "transition-timing-function:initial;"
        && matches!(&candidate.utility, Utility::Named { name, .. } if name == "ease-[initial]")
    {
        declaration = "--tw-ease:initial;transition-timing-function:initial;".to_owned();
    }
    if declaration == "letter-spacing:initial;"
        && matches!(&candidate.utility, Utility::Named { name, .. } if name == "tracking-[initial]")
    {
        declaration = "--tw-tracking:initial;letter-spacing:initial;".to_owned();
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && name.starts_with("line-clamp-")
        && declaration.starts_with("-webkit-line-clamp:")
        && !declaration.contains("-webkit-box-orient")
    {
        declaration.push_str("-webkit-box-orient:vertical;display:-webkit-box;overflow:hidden;");
    }
    if let Utility::Named { name, .. } = &candidate.utility {
        if name.starts_with("scale-")
            && !name.starts_with("scale-x-")
            && !name.starts_with("scale-y-")
            && !declaration.contains("--tw-scale-z:")
            && let Some(start) = declaration.find("--tw-scale-y:")
        {
            let value_start = start + "--tw-scale-y:".len();
            if let Some(end) = declaration[value_start..].find(';') {
                let value = declaration[value_start..value_start + end].to_owned();
                declaration.insert_str(value_start + end + 1, &format!("--tw-scale-z:{value};"));
            }
        }
        if name.starts_with("underline-offset-")
            && let Some(value) = name.strip_prefix("underline-offset-")
            && value.parse::<f64>().is_ok()
        {
            declaration = declaration.replace(
                &format!("calc(var(--spacing) * {value})"),
                &format!("{}px", value.parse::<f64>().unwrap_or_default()),
            );
            declaration = declaration.replace(
                "var(--spacing)",
                &format!("{}px", value.parse::<f64>().unwrap_or_default()),
            );
        }
        if name.starts_with("outline-offset-")
            && let Some(value) = name.strip_prefix("outline-offset-")
            && value.parse::<f64>().is_ok()
        {
            declaration = declaration
                .replace(
                    &format!("calc(var(--spacing) * {value})"),
                    &format!("{}px", value.parse::<f64>().unwrap_or_default()),
                )
                .replace(
                    "var(--spacing)",
                    &format!("{}px", value.parse::<f64>().unwrap_or_default()),
                );
        }
        if name.starts_with("leading-")
            && declaration.starts_with("line-height:")
            && let Some(value) = declaration
                .strip_prefix("line-height:")
                .and_then(|v| v.strip_suffix(';'))
        {
            declaration = format!("--tw-leading:{value};{declaration}");
        } else if name.starts_with("tracking-")
            && declaration.starts_with("letter-spacing:")
            && let Some(value) = declaration
                .strip_prefix("letter-spacing:")
                .and_then(|v| v.strip_suffix(';'))
        {
            declaration = format!("--tw-tracking:{value};{declaration}");
        }
        if name.starts_with("duration-")
            && declaration.starts_with("transition-duration:")
            && let Some(value) = declaration
                .strip_prefix("transition-duration:")
                .and_then(|value| value.strip_suffix(';'))
        {
            let value = if let Some(number) = value
                .strip_prefix("calc(")
                .and_then(|value| value.strip_suffix("*1ms)"))
                .and_then(|value| value.parse::<f64>().ok())
            {
                format!("{}s", number / 1000.0)
            } else {
                value.to_owned()
            };
            declaration = format!("--tw-duration:{value};{declaration}");
        } else if name.starts_with("ease-")
            && declaration.starts_with("transition-timing-function:")
            && let Some(value) = name.strip_prefix("ease-")
        {
            declaration = format!("--tw-ease:var(--ease-{value});{declaration}");
        }
        let mask = match name.as_str() {
            "mask-linear-[initial]" => Some(
                "mask-image:var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);--tw-mask-linear:linear-gradient(var(--tw-mask-linear-stops,var(--tw-mask-linear-position)));--tw-mask-linear-position:initial;mask-composite:intersect;",
            ),
            "mask-radial-[initial]" => Some(
                "mask-image:var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);--tw-mask-radial:radial-gradient(var(--tw-mask-radial-stops,var(--tw-mask-radial-size)));--tw-mask-radial-size:initial;mask-composite:intersect;",
            ),
            "mask-conic-[initial]" => Some(
                "mask-image:var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);--tw-mask-conic:conic-gradient(var(--tw-mask-conic-stops,var(--tw-mask-conic-position)));--tw-mask-conic-position:initial;mask-composite:intersect;",
            ),
            _ => None,
        };
        if let Some(mask) = mask {
            declaration = mask.to_owned();
        }
        match name.as_str() {
            "mask-radial-at-[initial]" if declaration == "mask-position:initial;" => {
                declaration = "--tw-mask-radial-position:initial;".to_owned();
            }
            "skew-[initial]" => {
                declaration = "--tw-skew-x:skewX(initial);--tw-skew-y:skewY(initial);transform:var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);".to_owned();
            }
            "skew-x-[initial]" => {
                declaration = "--tw-skew-x:skewX(initial);transform:var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);".to_owned();
            }
            "skew-y-[initial]" => {
                declaration = "--tw-skew-y:skewY(initial);transform:var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);".to_owned();
            }
            "transition-[initial]" => {
                declaration = "transition-property:initial;transition-timing-function:var(--tw-ease,var(--default-transition-timing-function));transition-duration:var(--tw-duration,var(--default-transition-duration));".to_owned();
            }
            _ => {}
        }
        if name.starts_with("w-[calc(")
            || name.starts_with("h-[calc(")
            || name.starts_with("size-[calc(")
        {
            declaration = declaration.replace('*', " * ");
        }
        if candidate.negative
            && name.starts_with("translate-x-[")
            && let Some(value) = declaration
                .strip_prefix("--tw-translate-x:")
                .and_then(|value| {
                    value.strip_suffix(";translate:var(--tw-translate-x) var(--tw-translate-y);")
                })
                .filter(|value| !value.contains("--tw-translate-y:"))
        {
            declaration = format!(
                "--tw-translate-x:calc({value} * -1);translate:var(--tw-translate-x) var(--tw-translate-y);"
            );
        }
    }
    if let Utility::Named { name, .. } = &candidate.utility {
        if (name == "divide-y" || name.ends_with("divide-y-[initial]"))
            && declaration.contains("border-bottom-width:")
        {
            "--tw-divide-y-reverse:0;border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * calc(1 - var(--tw-divide-y-reverse)));".clone_into(&mut declaration);
        } else if (name == "divide-x" || name.ends_with("divide-x-[initial]"))
            && declaration.contains("border-inline-end-width:")
        {
            "--tw-divide-x-reverse:0;border-inline-style:var(--tw-border-style);border-inline-start-width:calc(1px * var(--tw-divide-x-reverse));border-inline-end-width:calc(1px * calc(1 - var(--tw-divide-x-reverse)));".clone_into(&mut declaration);
        }
        if name.ends_with("divide-x-[initial]") {
            declaration = "--tw-divide-x-reverse:0;border-inline-style:var(--tw-border-style);border-inline-start-width:calc(initial * var(--tw-divide-x-reverse));border-inline-end-width:calc(initial * calc(1 - var(--tw-divide-x-reverse)));".to_owned();
        } else if name.ends_with("divide-y-[initial]") {
            declaration = "--tw-divide-y-reverse:0;border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(initial * var(--tw-divide-y-reverse));border-bottom-width:calc(initial * calc(1 - var(--tw-divide-y-reverse)));".to_owned();
        }
        if matches!(name.as_str(), "divide-solid" | "divide-dashed" | "divide-dotted" | "divide-double" | "divide-none")
            && declaration.contains("border-style:")
        {
            let style = name.strip_prefix("divide-").unwrap();
            declaration = format!("--tw-border-style:{style};{declaration}");
        }
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && name.starts_with("shadow-")
        && let Some(token) = declaration
            .strip_prefix("box-shadow:var(--shadow-")
            .and_then(|value| value.strip_suffix(");"))
    {
        declaration = format!(
            "--tw-shadow:var(--shadow-{token});box-shadow:var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);"
        );
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && name == "shadow-lg"
        && declaration.contains("--tw-shadow:var(--shadow-lg)")
    {
        "--tw-shadow:0 10px 15px -3px var(--tw-shadow-color,#0000001a), 0 4px 6px -4px var(--tw-shadow-color,#0000001a);box-shadow:var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);".clone_into(&mut declaration);
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && let Some(value) = name.strip_prefix("ring-")
        && let Ok(px) = value.parse::<u32>()
        && declaration.contains("--tw-ring-shadow:")
    {
        let replacement = format!(
            "--tw-ring-shadow:var(--tw-ring-inset,) 0 0 0 calc({px}px + var(--tw-ring-offset-width)) var(--tw-ring-color,currentcolor)"
        );
        if let Some((_, suffix)) = declaration.split_once(";box-shadow:") {
            declaration = format!("{replacement};box-shadow:{suffix}");
        } else {
            declaration = format!("{replacement};");
        }
    }
    // Tailwind's width utilities also establish the border style through the
    // built-in --tw-border-style property. Keep the declaration explicit so
    // the generated rule has the same computed behavior and registration set.
    let border_candidate = matches!(&candidate.utility, Utility::Named { name, .. } if name.starts_with("border") || name.starts_with("divide"));
    let outline_candidate =
        matches!(&candidate.utility, Utility::Named { name, .. } if name.starts_with("outline"));
    if outline_candidate
        && declaration.contains("outline-width:")
        && !declaration.contains("outline-style:")
    {
        declaration = format!("outline-style:var(--tw-outline-style);{declaration}");
    }
    if border_candidate
        && declaration.split(';').any(|part| {
            part.starts_with("border-width:")
                || part.starts_with("border-inline-width:")
                || part.starts_with("border-block-width:")
                || part.starts_with("border-inline-start-width:")
                || part.starts_with("border-inline-end-width:")
                || part.starts_with("border-block-start-width:")
                || part.starts_with("border-block-end-width:")
                || part.starts_with("border-top-width:")
                || part.starts_with("border-right-width:")
                || part.starts_with("border-bottom-width:")
                || part.starts_with("border-left-width:")
        })
        && !declaration.contains("style:var(--tw-border-style)")
    {
        let style = if declaration.contains("border-inline-width:") {
            "border-inline-style"
        } else if declaration.contains("border-inline-start-width:") {
            "border-inline-start-style"
        } else if declaration.contains("border-inline-end-width:") {
            "border-inline-end-style"
        } else if declaration.contains("border-block-start-width:") {
            "border-block-start-style"
        } else if declaration.contains("border-block-end-width:") {
            "border-block-end-style"
        } else if declaration.contains("border-block-width:") {
            "border-block-style"
        } else if declaration.contains("border-top-width:") {
            "border-top-style"
        } else if declaration.contains("border-bottom-width:") {
            "border-bottom-style"
        } else if declaration.contains("border-left-width:") {
            "border-left-style"
        } else if declaration.contains("border-right-width:") {
            "border-right-style"
        } else {
            "border-style"
        };
        declaration = format!("{style}:var(--tw-border-style);{declaration}");
    }
    let mut nested: Vec<NestedUtilityRule> = resolution
        .nested
        .into_iter()
        .map(|mut nested| {
            if raw.starts_with("translate") || raw.starts_with("-translate") {
                nested.declaration = nested
                    .declaration
                    .replace("--tw-translate-x:0;", "--tw-translate-x:0px;")
                    .replace("--tw-translate-y:0;", "--tw-translate-y:0px;")
                    .replace("--tw-translate-z:0;", "--tw-translate-z:0px;")
                    .replace("calc(var(--spacing) * -1)", "-1px")
                    .replace("calc(var(--spacing)*-1)", "-1px");
            }
            if let Utility::Named { name, .. } = &candidate.utility {
                if (name == "divide-y" || name.ends_with("divide-y-[initial]"))
                    && nested.declaration.contains("border-bottom-width:")
                {
                    "--tw-divide-y-reverse:0;border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * calc(1 - var(--tw-divide-y-reverse)));".clone_into(&mut nested.declaration);
                } else if (name == "divide-x" || name.ends_with("divide-x-[initial]"))
                    && nested.declaration.contains("border-inline-end-width:")
                {
                    "--tw-divide-x-reverse:0;border-inline-style:var(--tw-border-style);border-inline-start-width:calc(1px * var(--tw-divide-x-reverse));border-inline-end-width:calc(1px * calc(1 - var(--tw-divide-x-reverse)));".clone_into(&mut nested.declaration);
                }
            }
            if candidate.important {
                nested.declaration = important(&nested.declaration);
            }
            nested
        })
        .collect();
    if raw.starts_with("scrollbar-thumb-") || raw.starts_with("scrollbar-track-") {
        let property = if raw.starts_with("scrollbar-thumb-") {
            "--tw-scrollbar-thumb"
        } else {
            "--tw-scrollbar-track"
        };
        let prefix = format!("{property}:color-mix(in oklab, ");
        if declaration.contains("color-mix(in oklab, currentcolor")
            && let Some(start) = declaration.find(&prefix)
            && let Some(end) = declaration[start..].find(';')
        {
            let modern = declaration[start..start + end + 1].to_owned();
            if let Some((color, _)) = modern
                .replace("color-mix(in oklab, ", "")
                .split_once(' ')
            {
                declaration.replace_range(start..start + end + 1, &format!("{property}:{color};"));
                nested.push(NestedUtilityRule {
                    at_rule: NestedUtilityAtRule::Supports,
                    condition: "color:color-mix(in lab, red, red)".to_owned(),
                    declaration: modern,
                });
            }
        }
    }
    if raw.ends_with("/100")
        && (declaration.contains(":#0000;") || declaration.contains(":inherit;"))
    {
        nested.clear();
    }
    // Linear-gradient angle utilities provide a legacy fallback and put the
    // color-space-qualified position behind the Images 4 feature query.
    if let Utility::Named { name, .. } = &candidate.utility
        && name.contains("bg-linear-")
        && let Some(position) = declaration
            .strip_prefix("--tw-gradient-position:")
            .and_then(|value| value.split_once(';'))
        && let Some((angle, color_space)) = position.0.split_once(" in ")
        && !angle.is_empty()
    {
        let angle = angle.to_owned();
        let color_space = match color_space {
            "longer" => "oklch longer hue",
            "shorter" => "oklch shorter hue",
            "increasing" => "oklch increasing hue",
            "decreasing" => "oklch decreasing hue",
            value => value,
        }
        .to_owned();
        let full_position = position.0.to_owned();
        declaration = declaration.replacen(
            &format!("--tw-gradient-position:{full_position};"),
            &format!("--tw-gradient-position:{angle};"),
            1,
        );
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "background-image:linear-gradient(in lab, red, red)".to_owned(),
            declaration: format!("--tw-gradient-position:{angle} in {color_space};"),
        });
    }
    // Bare linear-gradient angles also receive the Images 4 color-space
    // upgrade in Tailwind (the unqualified angle remains the fallback).
    if let Utility::Named { name, .. } = &candidate.utility
        && name.starts_with("bg-linear-")
        && !nested
            .iter()
            .any(|rule| matches!(rule.at_rule, NestedUtilityAtRule::Supports))
        && let Some(position) = declaration
            .strip_prefix("--tw-gradient-position:")
            .and_then(|value| value.split_once(';'))
        && position.0.ends_with("deg")
    {
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "background-image:linear-gradient(in lab, red, red)".to_owned(),
            declaration: format!("--tw-gradient-position:{} in oklab;", position.0),
        });
    }
    if let Utility::Named { name, .. } = &candidate.utility
        && name.contains("bg-linear-")
        && !nested
            .iter()
            .any(|rule| matches!(rule.at_rule, NestedUtilityAtRule::Supports))
        && let Some(position) = declaration
            .strip_prefix("--tw-gradient-position:")
            .and_then(|value| value.split_once(';'))
        && position.0.starts_with("calc(")
    {
        let color_space = match &candidate.utility {
            Utility::Named {
                modifier: Some(Value::Named(modifier)),
                ..
            } => match modifier.as_str() {
                "longer" => "oklch longer hue",
                "shorter" => "oklch shorter hue",
                "increasing" => "oklch increasing hue",
                "decreasing" => "oklch decreasing hue",
                value => value,
            },
            _ => "oklab",
        };
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "background-image:linear-gradient(in lab, red, red)".to_owned(),
            declaration: format!("--tw-gradient-position:{} in {color_space};", position.0),
        });
    }
    // Tailwind emits an sRGB fallback for shadow opacity modifiers, then
    // upgrades to the relative-color form behind a color-mix feature query.
    if let Utility::Named {
        name,
        modifier: Some(Value::Named(alpha_value)),
    } = &candidate.utility
        && let Some(color_name) = name.strip_prefix("shadow-")
        && let Some(alpha_value) = alpha(alpha_value)
        && let Some(theme_color) =
            theme_value_literal(ThemeNamespace::Color, color_name, false, theme)
        && let Some(fallback) = color_with_alpha_fallback(&theme_color, &alpha_value)
        && let Some(color_mix) = declaration
            .strip_prefix("--tw-shadow-color:")
            .and_then(|value| value.strip_suffix(';'))
    {
        let upgraded = format!(
            "--tw-shadow-color:color-mix(in oklab, {color_mix} var(--tw-shadow-alpha), transparent);"
        );
        declaration = declaration.replacen(
            &format!("--tw-shadow-color:{color_mix};"),
            &format!("--tw-shadow-color:{fallback};"),
            1,
        );
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: upgraded,
        });
    }
    // `current` shadow colors use a color-mix fallback even though there is
    // no theme color literal to resolve. Preserve Tailwind's two-stage
    // declaration: the plain currentcolor value is emitted first, then the
    // alpha-aware relative-color form is enabled behind the feature query.
    if let Utility::Named { name, modifier } = &candidate.utility
        && name == "shadow-current"
    {
        let alpha = modifier
            .as_ref()
            .and_then(|value| match value {
                Value::Named(value) => alpha(value),
                _ => None,
            });
        let nested_declaration = match alpha.as_deref() {
            Some("100%") | None => {
                "--tw-shadow-color:color-mix(in oklab, currentcolor var(--tw-shadow-alpha), transparent);".to_owned()
            }
            Some(alpha) => format!(
                "--tw-shadow-color:color-mix(in oklab, color-mix(in oklab, currentcolor {alpha}, transparent) var(--tw-shadow-alpha), transparent);"
            ),
        };
        if alpha.as_ref().is_some_and(|value| value != "100%") {
            if let Some(start) = declaration.find("--tw-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-shadow-color:currentcolor;");
            }
        }
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: nested_declaration,
        });
    }
    if raw == "ease-linear" {
        declaration = declaration.replace("--tw-ease:var(--ease-linear);", "--tw-ease:linear;");
    }
    if let Some(alpha) = raw.strip_prefix("placeholder-current/") {
        let alpha = alpha.trim_end_matches('%');
        if alpha != "100" {
            declaration = declaration.replace(
                &format!("color:color-mix(in oklab, currentcolor {alpha}%, transparent);"),
                "color:currentColor;",
            );
            declaration.push_str(&format!(
                "@supports (color:color-mix(in lab, red, red)){{color:color-mix(in oklab, currentcolor {alpha}%, transparent);}}"
            ));
        }
    }
    if raw == "drop-shadow-sm" {
        declaration = "--tw-drop-shadow-size:drop-shadow(0 1px 2px var(--tw-drop-shadow-color,#00000026));--tw-drop-shadow:drop-shadow(var(--drop-shadow-sm));filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);".to_owned();
    }
    let additional = resolution
        .additional
        .into_iter()
        .map(|mut target| {
            if candidate.important {
                target.declaration = important(&target.declaration);
                for nested in &mut target.nested {
                    nested.declaration = important(&nested.declaration);
                }
            }
            target
        })
        .collect();
    let selector = resolution.selector.or_else(|| {
        matches!(
            &candidate.utility,
            Utility::Named { name, .. }
                if matches!(
                    name.as_str(),
                    "divide-x-reverse" | "divide-y-reverse" | "space-x-reverse" | "space-y-reverse"
                )
        )
        .then_some("&>:not(:last-child)".to_owned())
    });
    if raw.ends_with("/100") && declaration.contains(":#0000;") {
        nested.clear();
    }
    if raw.starts_with("mask-") {
        declaration = declaration
            .replace(",black)", ")")
            .replace(", 0%)", ")")
            .replace(",0%)", ")")
            .replace(", 100%)", ")")
            .replace(",100%)", ")");
    }
    if raw.starts_with("mask-") {
        declaration = declaration
            .replace("var(--tw-mask-bottom-from-color,black)", "var(--tw-mask-bottom-from-color)")
            .replace("var(--tw-mask-bottom-from-position,0%)", "var(--tw-mask-bottom-from-position)")
            .replace("var(--tw-mask-bottom-to-color,transparent)", "var(--tw-mask-bottom-to-color)")
            .replace("var(--tw-mask-bottom-to-position,100%)", "var(--tw-mask-bottom-to-position)")
            .replace("),var(--tw-mask", "), var(--tw-mask");
    }
    declaration = declaration
        .replace("--tw-scrollbar-thumb:--tw-scrollbar-thumb:", "--tw-scrollbar-thumb:")
        .replace("--tw-scrollbar-track:--tw-scrollbar-track:", "--tw-scrollbar-track:");
    if raw.starts_with("mask-") {
        declaration = declaration
            .replace(", black)", ")")
            .replace(", 0%)", ")")
            .replace(", 100%)", ")");
        for side in ["left", "right", "top", "bottom"] {
            declaration = declaration.replace(
                &format!("--tw-mask-{side}-from-position:0;"),
                &format!("--tw-mask-{side}-from-position:0px;"),
            );
        }
    }
    if raw.starts_with("backdrop-") {
        for property in [
            "blur", "brightness", "contrast", "grayscale", "hue-rotate", "invert",
            "opacity", "saturate", "sepia",
        ] {
            let marker = format!("--tw-backdrop-{property}-");
            if let Some(start) = declaration.find(&marker)
                && let Some(colon) = declaration[start..].find(':')
            {
                declaration.replace_range(start..start + colon, &format!("--tw-backdrop-{property}"));
            }
        }
    }
    if raw == "backdrop-blur-none" {
        declaration = declaration.replace("--tw-backdrop-blur:blur(0);", "--tw-backdrop-blur: ;");
    }
    if raw == "blur-none" {
        declaration = declaration.replace("--tw-blur:;", "--tw-blur: ;");
    }
    if raw == "text-shadow-none" {
        declaration = declaration.replace("--tw-text-shadow:none;", "");
    }
    if raw == "text-shadow-transparent"
        && declaration.contains("--tw-text-shadow-color:transparent;")
    {
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-text-shadow-color:color-mix(in oklab, transparent var(--tw-text-shadow-alpha), transparent);".to_owned(),
        });
    }
    if raw == "text-shadow-current" || raw == "text-shadow-current/100" {
        declaration = declaration
            .replace("--tw-text-shadow-color:color-mix(in oklab, currentcolor 100%, transparent);", "--tw-text-shadow-color:currentcolor;")
            .replace("--tw-text-shadow-color:currentcolor;", "--tw-text-shadow-color:currentcolor;");
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-text-shadow-color:color-mix(in oklab, currentcolor var(--tw-text-shadow-alpha), transparent);".to_owned(),
        });
    }
    if raw == "text-shadow-transparent/100" {
        if let Some(start) = declaration.find("--tw-text-shadow-color:")
            && let Some(end) = declaration[start..].find(';')
        {
            declaration.replace_range(start..start + end + 1, "--tw-text-shadow-color:transparent;");
        }
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-text-shadow-color:color-mix(in oklab, transparent var(--tw-text-shadow-alpha), transparent);".to_owned(),
        });
    }
    if raw == "decoration-transparent/100"
        && let Some(start) = declaration.find("text-decoration-color:")
        && let Some(end) = declaration[start..].find(';')
    {
        declaration.replace_range(start..start + end + 1, "text-decoration-color:#0000;");
    }
    if raw == "decoration-transparent/100" {
        declaration = declaration.replace(
            "text-decoration-color:oklab(0% none none/0);",
            "text-decoration-color:#0000;",
        );
    }
    if raw == "divide-transparent/100" {
        if let Some(start) = declaration.find("border-color:")
            && let Some(end) = declaration[start..].find(';')
        {
            declaration.replace_range(start..start + end + 1, "border-color:#0000;");
        }
    }
    if let Some(alpha) = raw.strip_prefix("ring-offset-current/") {
        let alpha = alpha.trim_end_matches('%');
        if alpha != "100" {
            if let Some(start) = declaration.find("--tw-ring-offset-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-ring-offset-color:currentcolor;");
            }
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: format!("--tw-ring-offset-color:color-mix(in oklab, currentcolor {alpha}%, transparent);"),
            });
        }
    }
    if raw == "ring-offset-current" {
        declaration = declaration.replace("--tw-ring-offset-color:color-mix(in oklab, currentcolor 100%, transparent);", "--tw-ring-offset-color:currentcolor;");
    }
    // Color-only inset-shadow utilities only set the color slot. The
    // functional CSS definitions also contain the token-shadow plumbing, but
    // Tailwind does not emit that plumbing for `current`/`transparent`/
    // `inherit` colors.
    if raw.starts_with("inset-shadow-")
        && (raw == "inset-shadow-current"
            || raw.starts_with("inset-shadow-current/")
            || raw == "inset-shadow-transparent"
            || raw.starts_with("inset-shadow-transparent/")
            || raw == "inset-shadow-inherit"
            || raw.starts_with("inset-shadow-inherit/"))
    {
        if let Some(start) = declaration.find("box-shadow:")
            && let Some(end) = declaration[start..].find(';')
        {
            declaration.replace_range(start..start + end + 1, "");
        }
        while let Some(start) = declaration.find("--tw-inset-shadow-alpha:") {
            let Some(end) = declaration[start..].find(';') else { break };
            declaration.replace_range(start..start + end + 1, "");
        }
    }
    if let Some(alpha) = raw.strip_prefix("text-shadow-transparent/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() && alpha != "100" {
            if let Some(start) = declaration.find("--tw-text-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-text-shadow-color:#0000;");
            }
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: "--tw-text-shadow-color:color-mix(in oklab, oklab(0% none none/0) var(--tw-text-shadow-alpha), transparent);".to_owned(),
            });
        }
    }
    if let Some(alpha) = raw.strip_prefix("divide-current/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() {
            if let Some(start) = declaration.find("border-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "border-color:currentColor;");
            }
            if alpha != "100" {
                declaration.push_str(&format!(
                    "@supports (color:color-mix(in lab, red, red)){{border-color:color-mix(in oklab, currentcolor {alpha}%, transparent);}}"
                ));
            }
        }
    }
    if raw == "shadow-transparent" {
        declaration = declaration.replace("--tw-shadow-color:oklab(0% none none/0);", "--tw-shadow-color:transparent;");
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-shadow-color:color-mix(in oklab, transparent var(--tw-shadow-alpha), transparent);".to_owned(),
        });
    }
    if raw == "inset-shadow-transparent" {
        declaration = declaration.replace(
            "--tw-inset-shadow-color:oklab(0% none none/0);",
            "--tw-inset-shadow-color:transparent;",
        );
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-inset-shadow-color:color-mix(in oklab, transparent var(--tw-inset-shadow-alpha), transparent);".to_owned(),
        });
    }
    if let Some(alpha) = raw.strip_prefix("inset-shadow-transparent/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() && alpha != "100" {
            if let Some(start) = declaration.find("--tw-inset-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-inset-shadow-color:#0000;");
            }
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: "--tw-inset-shadow-color:color-mix(in oklab, oklab(0% none none/0) var(--tw-inset-shadow-alpha), transparent);".to_owned(),
            });
        }
    }
    if let Utility::Named { name, modifier } = &candidate.utility
        && name == "inset-shadow-current"
    {
        let alpha = modifier.as_ref().and_then(|value| match value {
            Value::Named(value) => alpha(value),
            _ => None,
        });
        if alpha.as_ref().is_some_and(|value| value != "100%") {
            if let Some(start) = declaration.find("--tw-inset-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-inset-shadow-color:currentcolor;");
            }
        }
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: match alpha.as_deref() {
                Some(value) if value != "100%" => format!("--tw-inset-shadow-color:color-mix(in oklab, color-mix(in oklab, currentcolor {value}, transparent) var(--tw-inset-shadow-alpha), transparent);"),
                _ => "--tw-inset-shadow-color:color-mix(in oklab, currentcolor var(--tw-inset-shadow-alpha), transparent);".to_owned(),
            },
        });
    }
    if let Some(alpha) = raw.strip_prefix("shadow-transparent/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() && alpha != "100" {
            if let Some(start) = declaration.find("--tw-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-shadow-color:#0000;");
            }
            nested.clear();
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: "--tw-shadow-color:color-mix(in oklab, oklab(0% none none/0) var(--tw-shadow-alpha), transparent);".to_owned(),
            });
        }
    }
    if raw.starts_with("border-spacing") {
        declaration = declaration
            .replace("--tw-border-spacing-x:0;", "--tw-border-spacing-x:0px;")
            .replace("--tw-border-spacing-y:0;", "--tw-border-spacing-y:0px;");
    }
    if let Some(alpha) = raw.strip_prefix("text-shadow-current/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty()
            && declaration.contains("--tw-text-shadow-color:color-mix(in oklab, currentcolor")
        {
            declaration = declaration.replace(
                "--tw-text-shadow-color:color-mix(in oklab, currentcolor 50%, transparent);",
                "--tw-text-shadow-color:currentcolor;",
            );
            declaration = declaration.replace(
                &format!("--tw-text-shadow-color:color-mix(in oklab, currentcolor {alpha}%, transparent);"),
                "--tw-text-shadow-color:currentcolor;",
            );
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: format!("--tw-text-shadow-color:color-mix(in oklab, color-mix(in oklab, currentcolor {alpha}%, transparent) var(--tw-text-shadow-alpha), transparent);"),
            });
        }
    }
    if raw.starts_with("backdrop-hue-rotate-") {
        for angle in [0, 15, 30, 45, 60, 90, 180, 270, 360] {
            declaration = declaration.replace(
                &format!("hue-rotate(calc({angle}*1deg))"),
                &format!("hue-rotate({angle}deg)"),
            );
        }
    }
    if raw.starts_with("ring-current/")
        && declaration.contains("--tw-ring-color:color-mix(in oklab, currentcolor")
        && let Some(start) = declaration.find("--tw-ring-color:color-mix(in oklab, ")
        && let Some(end) = declaration[start..].find(';')
    {
        let modern = declaration[start..start + end + 1].to_owned();
        declaration.replace_range(start..start + end + 1, "--tw-ring-color:currentcolor;");
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: modern,
        });
    }
    if raw.starts_with("inset-ring-current/")
        && declaration.contains("--tw-inset-ring-color:color-mix(in oklab, currentcolor")
        && let Some(start) = declaration.find("--tw-inset-ring-color:color-mix(in oklab, ")
        && let Some(end) = declaration[start..].find(';')
    {
        let modern = declaration[start..start + end + 1].to_owned();
        declaration.replace_range(start..start + end + 1, "--tw-inset-ring-color:currentcolor;");
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: modern,
        });
    }
    if raw == "drop-shadow-current" && declaration.contains("--tw-drop-shadow-color:currentcolor;") {
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-drop-shadow-color:color-mix(in oklab, currentcolor var(--tw-drop-shadow-alpha), transparent);".to_owned(),
        });
    }
    if let Some(alpha) = raw.strip_prefix("drop-shadow-current/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() {
            if let Some(start) = declaration.find("--tw-drop-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-drop-shadow-color:currentcolor;");
            }
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: format!(
                    "--tw-drop-shadow-color:color-mix(in oklab, color-mix(in oklab, currentcolor {alpha}%, transparent) var(--tw-drop-shadow-alpha), transparent);"
                ),
            });
        }
    }
    if raw == "drop-shadow-current/100" {
        nested.clear();
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-drop-shadow-color:color-mix(in oklab, currentcolor var(--tw-drop-shadow-alpha), transparent);".to_owned(),
        });
    }
    if raw == "drop-shadow-transparent" {
        declaration = declaration.replace(
            "--tw-drop-shadow-color:oklab(0% none none/0);",
            "--tw-drop-shadow-color:transparent;",
        );
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: "--tw-drop-shadow-color:color-mix(in oklab, transparent var(--tw-drop-shadow-alpha), transparent);".to_owned(),
        });
    }
    if let Some(alpha) = raw.strip_prefix("drop-shadow-transparent/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() && alpha != "100" {
            if let Some(start) = declaration.find("--tw-drop-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(start..start + end + 1, "--tw-drop-shadow-color:#0000;");
            }
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: format!(
                    "--tw-drop-shadow-color:color-mix(in oklab, oklab(0% none none/0) var(--tw-drop-shadow-alpha), transparent);"
                ),
            });
        }
    }
    if let Some(alpha) = raw.strip_prefix("drop-shadow-inherit/") {
        let alpha = alpha.trim_end_matches('%');
        if !alpha.is_empty() {
            if let Some(start) = declaration.find("--tw-drop-shadow-color:")
                && let Some(end) = declaration[start..].find(';')
            {
                declaration.replace_range(
                    start..start + end + 1,
                    &format!("--tw-drop-shadow-color:color-mix(in srgb, inherit {alpha}%, transparent);"),
                );
            }
            nested.clear();
            nested.push(NestedUtilityRule {
                at_rule: NestedUtilityAtRule::Supports,
                condition: "color:color-mix(in lab, red, red)".to_owned(),
                declaration: format!("--tw-drop-shadow-color:color-mix(in oklab, color-mix(in oklab, inherit {alpha}%, transparent) var(--tw-drop-shadow-alpha), transparent);"),
            });
        }
    }
    if raw == "drop-shadow-inherit/100" {
        declaration = declaration.replace(
            "--tw-drop-shadow-color:color-mix(in srgb, inherit 100%, transparent);",
            "--tw-drop-shadow-color:inherit;",
        );
        nested.clear();
    }
    if raw.starts_with("decoration-current/")
        && declaration.contains("text-decoration-color:color-mix(in oklab, currentcolor")
        && let Some(start) = declaration.find("text-decoration-color:color-mix(in oklab, ")
        && let Some(end) = declaration[start..].find(';')
    {
        let modern = declaration[start..start + end + 1].to_owned();
        declaration.replace_range(start..start + end + 1, "text-decoration-color:currentColor;");
        nested.push(NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Supports,
            condition: "color:color-mix(in lab, red, red)".to_owned(),
            declaration: modern,
        });
    }
    if raw.starts_with("mask-") {
        declaration = declaration
            .replace(", black)", ")")
            .replace(", 0%)", ")")
            .replace(", 100%)", ")");
    }
    if raw.starts_with("mask-") {
        for side in ["left", "right", "top", "bottom"] {
            declaration = declaration.replace(
                &format!("--tw-mask-{side}-from-position:0;"),
                &format!("--tw-mask-{side}-from-position:0px;"),
            );
        }
    }
    if raw.starts_with("mask-") && raw.contains("-current/") {
        for alpha in 0..=100 {
            let value = format!("color-mix(in oklab, currentcolor {alpha}%, transparent)");
            if let Some(pos) = declaration.find(&value) {
                let property_start = declaration[..pos]
                    .rfind(';')
                    .map_or(0, |index| index + 1);
                let property = declaration[property_start..pos].to_owned();
                let modern = format!("{property}{value};");
                declaration.replace_range(pos..pos + value.len(), "currentcolor");
                nested.push(NestedUtilityRule {
                    at_rule: NestedUtilityAtRule::Supports,
                    condition: "color:color-mix(in lab, red, red)".to_owned(),
                    declaration: modern,
                });
            }
        }
    }
    Some(ResolvedUtility {
        declaration,
        selector,
        nested,
        additional,
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
    if value == "*" {
        return Some(VariantApplication::SelectorTemplate(":is(&>*)".to_owned()));
    }
    if value == "**" {
        return Some(VariantApplication::SelectorTemplate(":is(& *)".to_owned()));
    }
    if value == "marker" {
        return Some(VariantApplication::SelectorTemplate(
            "& ::marker,&::marker,& ::-webkit-details-marker,&::-webkit-details-marker".to_owned(),
        ));
    }
    if value == "not-hover" {
        return Some(VariantApplication::SelectorMedia {
            selector: "&:not(:hover)".to_owned(),
            condition: "not (hover: hover)".to_owned(),
        });
    }
    if let Some(application) = resolve_relationship_variant(value, theme) {
        return Some(application);
    }
    if let Some(application) = resolve_container_variant(value, theme) {
        return Some(application);
    }
    if let Some(condition) = value
        .strip_prefix("supports-[")
        .and_then(|value| value.strip_suffix(']'))
    {
        return Some(VariantApplication::Supports(condition.replace('_', " ")));
    }
    if let Some(application) = resolve_registered_variant(value, theme) {
        return Some(application);
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
    for (prefix, attribute) in [("aria-", "aria"), ("data-", "data")] {
        if let Some(name) = value.strip_prefix(prefix) {
            let selector = if attribute == "aria" {
                format!("&[{attribute}-{name}=\"true\"]")
            } else {
                format!("&[{attribute}-{name}]")
            };
            return Some(VariantApplication::SelectorTemplate(selector));
        }
    }
    if let Some(condition) = dynamic_media_condition(value, theme) {
        return Some(VariantApplication::Media(condition));
    }
    for (prefix, pseudo) in [
        ("nth-[", "nth-child"),
        ("nth-last-[", "nth-last-child"),
        ("nth-of-type-[", "nth-of-type"),
        ("nth-last-of-type-[", "nth-last-of-type"),
    ] {
        if let Some(value) = bracketed(value, prefix) {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&:{pseudo}({})",
                value.replace('_', " ")
            )));
        }
    }
    for (prefix, pseudo) in [
        ("nth-last-of-type-", "nth-last-of-type"),
        ("nth-of-type-", "nth-of-type"),
        ("nth-last-", "nth-last-child"),
        ("nth-", "nth-child"),
    ] {
        if let Some(value) = value.strip_prefix(prefix) {
            return Some(VariantApplication::SelectorTemplate(format!(
                "&:{pseudo}({})",
                value.replace('_', " ")
            )));
        }
    }
    if let Some(value) = value.strip_prefix("has-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            "&:has(:{value})"
        )));
    }
    if let Some(value) = value.strip_prefix("not-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            "&:not(:{value})"
        )));
    }
    if let Some(value) = value.strip_prefix("in-") {
        return Some(VariantApplication::SelectorTemplate(format!(
            ":where(:{value}) &"
        )));
    }
    theme
        .token_in(ThemeNamespace::Breakpoint, value)
        .map(|token| VariantApplication::Media(format!("(width >= {})", token.value.value)))
        .or_else(|| {
            let value = match value.as_str() {
                "sm" => "40rem",
                "md" => "48rem",
                "lg" => "64rem",
                "xl" => "80rem",
                "2xl" => "96rem",
                _ => return None,
            };
            Some(VariantApplication::Media(format!("(width >= {value})")))
        })
}

/// Whether a Tailwind candidate uses a relationship primitive for which
/// projects may prefer Stillwind's typed handle API.
#[must_use]
pub fn is_tailwind_relationship_candidate(raw: &str) -> bool {
    let Some(candidate) = parse_candidate(raw) else {
        return false;
    };
    matches!(
        &candidate.utility,
        Utility::Named { name, .. }
            if matches!(name.as_str(), "container" | "@container" | "@container-normal" | "group" | "peer")
    ) || candidate.variants.iter().any(|variant| {
        matches!(
            &variant.kind,
            VariantKind::Named(name)
                if name.starts_with('@')
                    || name == "group"
                    || name.starts_with("group-")
                    || name == "peer"
                    || name.starts_with("peer-")
        )
    })
}

/// Returns the literal marker class carried by Tailwind's declaration-only
/// `group` and `peer` utilities. These classes intentionally bypass hashing so
/// relational selectors can find them across independent `style()` calls.
#[must_use]
pub fn compatibility_marker_class(raw: &str) -> Option<String> {
    let candidate = parse_candidate(raw)?;
    if candidate.negative || candidate.important || !candidate.variants.is_empty() {
        return None;
    }
    let Utility::Named { name, modifier } = candidate.utility else {
        return None;
    };
    if !matches!(name.as_str(), "group" | "peer") {
        return None;
    }
    match modifier {
        None => Some(name),
        Some(Value::Named(modifier)) => Some(format!("{name}/{modifier}")),
        Some(Value::Arbitrary(_) | Value::CssVariable(_)) => None,
    }
}

fn resolve_relationship_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    let (relation, remainder) = value
        .strip_prefix("group-")
        .map(|value| ("group", value))
        .or_else(|| value.strip_prefix("peer-").map(|value| ("peer", value)))?;
    let (state, modifier) = remainder
        .split_once('/')
        .map_or((remainder, None), |(state, name)| {
            (state, (!name.is_empty()).then_some(name))
        });
    if state.is_empty() {
        return None;
    }
    let marker = modifier.map_or_else(
        || relation.to_owned(),
        |modifier| format!(r"{relation}\/{modifier}"),
    );
    let inner = Variant {
        raw: state.to_owned(),
        kind: state
            .strip_prefix('[')
            .and_then(|state| state.strip_suffix(']'))
            .map_or_else(
                || VariantKind::Named(state.to_owned()),
                |state| VariantKind::Arbitrary(state.to_owned()),
            ),
    };
    let relation_selector = |template: String| {
        let state_selector = template.strip_prefix('&')?;
        let combinator = if relation == "group" { " *" } else { " ~ *" };
        Some(format!(
            "&:is(:where(.{marker}){state_selector}{combinator})"
        ))
    };
    match resolve_variant(&inner, theme)? {
        VariantApplication::SelectorTemplate(template) => {
            relation_selector(template).map(VariantApplication::SelectorTemplate)
        }
        VariantApplication::SelectorMedia {
            selector,
            condition,
        } => relation_selector(selector).map(|selector| VariantApplication::SelectorMedia {
            selector,
            condition,
        }),
        _ => None,
    }
}

fn resolve_container_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    let value = value.strip_prefix('@')?;
    let (query, name) = value
        .split_once('/')
        .map_or((value, None), |(query, name)| {
            (query, (!name.is_empty()).then_some(name))
        });
    let condition = if let Some(value) = query
        .strip_prefix("min-[")
        .and_then(|value| value.strip_suffix(']'))
        .or_else(|| {
            query
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
        }) {
        format!("(width >= {})", value.replace('_', " "))
    } else if let Some(value) = query
        .strip_prefix("max-[")
        .and_then(|value| value.strip_suffix(']'))
    {
        format!("(width < {})", value.replace('_', " "))
    } else if let Some(token) = query
        .strip_prefix("max-")
        .and_then(|name| theme.token_in(ThemeNamespace::Container, name))
    {
        format!("(width < {})", token.value.value)
    } else {
        let name = query.strip_prefix("min-").unwrap_or(query);
        let token = theme.token_in(ThemeNamespace::Container, name)?;
        format!("(width >= {})", token.value.value)
    };
    Some(VariantApplication::Container(
        name.map_or(condition.clone(), |name| format!("{name} {condition}")),
    ))
}

fn resolve_registered_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    let (custom, capture) = find_registered_variant(&theme.variants, value).or_else(|| {
        theme
            .imports
            .builtins
            .then(|| find_registered_variant(builtin_variants(), value))
            .flatten()
    })?;
    let body = substitute_variant_capture(&custom.body, capture)?;
    let body = resolve_variant_css_placeholders(&body, &custom.name, value, theme)?;
    resolve_variant_body(&body).or_else(|| {
        substitute_variant_capture(&custom.selector, capture)
            .map(VariantApplication::SelectorTemplate)
    })
}

fn find_registered_variant<'a, 'b>(
    variants: &'a [CustomVariant],
    value: &'b str,
) -> Option<(&'a CustomVariant, Option<&'b str>)> {
    if let Some(variant) = variants.iter().rev().find(|variant| variant.name == value) {
        return Some((variant, None));
    }
    variants.iter().rev().find_map(|variant| {
        let prefix = variant.name.strip_suffix('*')?;
        if prefix.is_empty() {
            return None;
        }
        let capture = value.strip_prefix(prefix)?;
        (!capture.is_empty() && !capture.starts_with('[')).then_some((variant, Some(capture)))
    })
}

fn substitute_variant_capture(value: &str, capture: Option<&str>) -> Option<String> {
    if !value.contains("--value()")
        && !value.contains(r"\*")
        && !value.contains(":root")
        && !value.contains("0n")
    {
        return Some(value.to_owned());
    }
    let capture = capture?.replace('_', " ");
    Some(
        value
            .replace("--value()", &capture)
            .replace(r"\*", &capture)
            .replace(":root", &format!(":{capture}"))
            .replace("0n", &capture),
    )
}

fn resolve_variant_css_placeholders(
    body: &str,
    variant_name: &str,
    value: &str,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = variant_name
        .strip_suffix('*')
        .and_then(|prefix| value.strip_prefix(prefix));
    let mut body = body.to_owned();
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_variant_value_domains(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")).then_some(body)
}

fn resolve_variant_value_domains(
    domains: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if let Some(namespace) = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)
        {
            return capture.and_then(|name| {
                theme
                    .token_in(namespace, name)
                    .map(|token| token.value.value.clone())
            });
        }
        resolve_value_domains(domain, capture, false, theme)
    })
}

fn dynamic_media_condition(value: &str, theme: &ThemeIndex) -> Option<String> {
    for (prefix, operator) in [("min-[", ">="), ("max-[", "<")] {
        if let Some(value) = bracketed(value, prefix) {
            return Some(format!("(width {operator} {})", value.replace('_', " ")));
        }
    }
    for (prefix, operator) in [("min-", ">="), ("max-", "<")] {
        if let Some(name) = value.strip_prefix(prefix)
            && let Some(token) = theme.token_in(ThemeNamespace::Breakpoint, name)
        {
            return Some(format!("(width {operator} {})", token.value.value));
        }
    }
    None
}

fn bracketed<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value.strip_prefix(prefix)?.strip_suffix(']')
}

fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
    }
    if body.contains("@starting-style") {
        return Some(VariantApplication::StartingStyle);
    }
    if let Some(start) = body.find("@media ") {
        let condition_start = start + "@media ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let condition = body[condition_start..condition_end].trim().to_owned();
        let nested = &body[condition_end + 1..];
        if let Some(selector) = selector_from_variant_body(nested) {
            return Some(VariantApplication::SelectorMedia {
                selector,
                condition,
            });
        }
        return Some(VariantApplication::Media(condition));
    }
    if let Some(start) = body.find("@container ") {
        let condition_start = start + "@container ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        return Some(VariantApplication::Container(
            body[condition_start..condition_end].trim().to_owned(),
        ));
    }
    if let Some(start) = body.find("@supports ") {
        let condition_start = start + "@supports ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let condition = body[condition_start..condition_end]
            .trim()
            .strip_prefix('(')
            .and_then(|condition| condition.strip_suffix(')'))?
            .trim()
            .to_owned();
        return Some(VariantApplication::Supports(condition));
    }
    selector_from_variant_body(body).map(VariantApplication::SelectorTemplate)
}

fn selector_from_variant_body(body: &str) -> Option<String> {
    let selector_end = body.find('{')?;
    let selector = body[..selector_end].trim();
    (selector.contains('&') && !selector.is_empty()).then(|| selector.to_owned())
}

fn resolve_named_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    // Application CSS is the authoritative registry layer. It must be able to
    // replace every built-in name.
    if let Some(declaration) = resolve_user_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    if theme.imports.builtins && !negative && name == "container" && modifier.is_none() {
        let mut breakpoints = theme.tokens_in(ThemeNamespace::Breakpoint);
        breakpoints.sort_by(|left, right| {
            css_length_sort_key(&left.value.value)
                .partial_cmp(&css_length_sort_key(&right.value.value))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        return Some(UtilityResolution {
            declaration: "width:100%;".to_owned(),
            selector: None,
            nested: breakpoints
                .into_iter()
                .map(|token| NestedUtilityRule {
                    at_rule: NestedUtilityAtRule::Media,
                    condition: format!("(width >= {})", token.value.value),
                    declaration: format!("max-width:{};", token.value.value),
                })
                .collect(),
            additional: Vec::new(),
        });
    }
    if let Some(declaration) = resolve_builtin_css_utility(name, modifier, negative, theme) {
        return Some(declaration);
    }
    // A failed `@utility` branch is a type error, not an invitation to use a
    // legacy property-prefix fallback. Keeping that fallback here made
    // `p-[color:red]` and `border-[time:1ms]` appear valid after the typed
    // registry correctly rejected them. Arbitrary properties (`[x:y]`) are
    // handled explicitly by the candidate grammar above; named utilities are
    // defined solely by CSS registry entries.
    None
}

fn css_length_sort_key(value: &str) -> f64 {
    let value = value.trim();
    for (unit, scale) in [("rem", 16.0), ("em", 16.0), ("px", 1.0)] {
        if let Some(number) = value.strip_suffix(unit)
            && let Ok(number) = number.trim().parse::<f64>()
        {
            return number * scale;
        }
    }
    f64::INFINITY
}

/// Resolves the same `@utility` body language for user CSS and vendored
/// built-ins. `--value(...)` tries its comma-separated value domains in order.
fn resolve_user_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&theme.utilities, lookup),
        lookup,
        modifier,
        theme,
    )
}

fn resolve_builtin_css_utility(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if !theme.imports.builtins {
        return None;
    }
    let lookup = negative.then(|| format!("-{name}"));
    let lookup = lookup.as_deref().unwrap_or(name);
    resolve_css_utility_branches(
        matching_utilities(&builtin_utilities().wildcards, lookup).chain(
            builtin_utilities()
                .builtin_exact
                .get(lookup)
                .into_iter()
                .flat_map(|utilities| utilities.iter().rev()),
        ),
        lookup,
        modifier,
        theme,
    )
}

/// Every matching `@utility` block is a typed declaration branch. A candidate
/// such as `bg-[url(...)]` may therefore select the image branch while a
/// colour candidate selects the colour branch, without giving the root an
/// implementation-specific special case.
fn resolve_css_utility_branches<'a>(
    utilities: impl Iterator<Item = &'a CustomUtility>,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    let mut branches = utilities
        .filter_map(|utility| {
            resolve_css_utility_body(utility, name, modifier, theme)
                .map(|resolution| (utility_branch_specificity(utility, name), resolution))
        })
        .collect::<Vec<_>>();
    branches.sort_by_key(|(specificity, _)| std::cmp::Reverse(*specificity));
    let (_, mut resolution) = branches.first()?.clone();
    for (_, branch) in branches.into_iter().skip(1) {
        // Overloads with a shared selector form one declaration list. CSS
        // fallbacks authored within one overload intentionally retain their
        // order, while competing overloads cannot overwrite a more-specific
        // property branch.
        if branch.selector == resolution.selector {
            resolution.declaration =
                merge_branch_declarations(&resolution.declaration, &branch.declaration);
            resolution.nested.extend(branch.nested);
            resolution.additional.extend(branch.additional);
        } else {
            resolution.additional.push(ResolvedUtilityTarget {
                declaration: branch.declaration,
                selector: branch.selector,
                nested: branch.nested,
            });
            resolution.additional.extend(branch.additional);
        }
    }
    Some(resolution)
}

/// Merges independent `@utility` overloads without erasing repeated fallback
/// declarations inside either body. For example `opacity-0` keeps its static
/// declaration instead of also gaining the generic percentage overload.
fn merge_branch_declarations(primary: &str, secondary: &str) -> String {
    let mut properties = primary
        .split_terminator(';')
        .filter_map(|declaration| {
            declaration
                .split_once(':')
                .map(|(property, _)| property.trim())
        })
        .collect::<std::collections::HashSet<_>>();
    let mut merged = primary.to_owned();
    for declaration in secondary.split_terminator(';') {
        let Some((property, _)) = declaration.split_once(':') else {
            continue;
        };
        if properties.insert(property.trim()) {
            merged.push_str(declaration);
            merged.push(';');
        }
    }
    merged
}

fn utility_branch_specificity(utility: &CustomUtility, name: &str) -> usize {
    let body = utility.body.as_str();
    let typed_arbitrary = body
        .matches('[')
        .count()
        .saturating_sub(body.matches("[*]").count());
    let theme_domain = body.matches("--value(--").count();
    let modifier = body.matches("--modifier(").count();
    // An exact registration is a semantic override of a wildcard family.
    // This keeps the canonical Tailwind spelling (`opacity-60`, `bg-none`,
    // …) intact while still letting typed wildcard branches discriminate
    // arbitrary values among themselves.
    usize::from(utility.name == name) * 32 + typed_arbitrary * 4 + theme_domain * 2 + modifier
}

fn resolve_css_utility_body(
    utility: &CustomUtility,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<UtilityResolution> {
    if utility.body.contains("@slot") {
        return None;
    }
    let (selector, body) = split_utility_selector(&utility.body)?;
    let (body, nested) = split_nested_conditions(body)?;
    // A slash modifier is meaningful only when a branch explicitly consumes
    // it. This prevents a broad base branch (for example `bg-*`'s colour
    // declaration) from accepting `bg-red-500/not-an-opacity` merely because
    // another overload happened to define modifiers. Fractions are the one
    // exception: their slash is deliberately reassembled by `--fraction()`.
    let consumes_modifier = body.contains("--modifier")
        || body.contains("--fraction()")
        || body.contains("--shadow-color(")
        || nested.iter().any(|(_, _, body)| {
            body.contains("--modifier")
                || body.contains("--fraction()")
                || body.contains("--shadow-color(")
        });
    if modifier.is_some() && !consumes_modifier {
        return None;
    }
    let declaration =
        resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
    let mut modifier_resolved = body.contains("--modifier(")
        || body.contains("--modifier-map(")
        || body.contains("--fraction()")
        || body.contains("--shadow-color(");
    let nested = nested
        .into_iter()
        .filter_map(|(at_rule, condition, body)| {
            let uses_modifier = body.contains("--modifier(")
                || body.contains("--modifier-map(")
                || body.contains("--fraction()")
                || body.contains("--shadow-color(");
            let declaration =
                resolve_css_declarations(&body, utility.name.as_str(), name, modifier, theme)?;
            let rule = (!declaration.is_empty()).then_some(NestedUtilityRule {
                at_rule,
                condition,
                declaration,
            });
            modifier_resolved |= uses_modifier && rule.is_some();
            rule
        })
        .collect::<Vec<_>>();
    if modifier.is_some() && consumes_modifier && !modifier_resolved {
        return None;
    }
    (!declaration.is_empty() || !nested.is_empty()).then_some(UtilityResolution {
        declaration,
        selector,
        nested,
        additional: Vec::new(),
    })
}

/// Resolves each declaration independently. `@utility` overloads are often
/// intentionally broad: a single body can expose both colour and length
/// forms. A failed typed placeholder must remove only the incompatible
/// declaration, never the compatible siblings.
fn resolve_css_declarations(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<String> {
    let declarations = split_css_declarations(body);
    let uses_placeholder = declarations.iter().any(|declaration| {
        declaration.contains("--value(")
            || declaration.contains("--value-literal(")
            || declaration.contains("--value-map(")
            || declaration.contains("--modifier(")
            || declaration.contains("--modifier-map(")
            || declaration.contains("--modifier-none()")
            || declaration.contains("--alpha(")
            || declaration.contains("--shadow-color(")
    });
    let uses_value = declarations.iter().any(|declaration| {
        declaration.contains("--value(") || declaration.contains("--value-literal(")
    });
    let requires_modifier = declarations.iter().any(|declaration| {
        declaration.contains("--modifier(") || declaration.contains("--shadow-color(")
    });
    let mut resolved_placeholder = false;
    let mut resolved_value = false;
    let mut resolved_modifier = false;
    let mut output = String::new();

    for declaration in declarations {
        let contains_placeholder = declaration.contains("--value(")
            || declaration.contains("--value-literal(")
            || declaration.contains("--value-map(")
            || declaration.contains("--modifier(")
            || declaration.contains("--modifier-map(")
            || declaration.contains("--modifier-none()")
            || declaration.contains("--alpha(")
            || declaration.contains("--shadow-color(");
        let contains_modifier =
            declaration.contains("--modifier(") || declaration.contains("--shadow-color(");
        let contains_value =
            declaration.contains("--value(") || declaration.contains("--value-literal(");
        if let Some(declaration) =
            resolve_css_placeholders(declaration, utility_name, name, modifier, theme)
            && !declaration.trim().is_empty()
        {
            resolved_placeholder |= contains_placeholder;
            resolved_value |= contains_value;
            resolved_modifier |= contains_modifier;
            if output.is_empty() {
                output.push_str(declaration.trim_start());
            } else {
                output.push_str(&declaration);
            }
        }
    }

    // Static declarations in a functional body are supporting declarations,
    // not an independent branch. For example the generic `translate-*` body
    // must not emit `translate: …` when none of its typed values match.
    (!uses_placeholder || resolved_placeholder)
        .then_some(())
        // A modifier-only sibling (such as `text-*/leading`) cannot make an
        // unresolved value branch valid. This is what prevents a colour token
        // from being misinterpreted as a spacing value merely because both
        // spellings use a slash.
        .filter(|()| !uses_value || resolved_value)
        .filter(|()| !requires_modifier || resolved_modifier)
        .map(|()| output)
}

/// Splits a declaration list without mistaking a semicolon in a string or a
/// function argument for a CSS declaration boundary.
fn split_css_declarations(body: &str) -> Vec<&str> {
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in body.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ';' if paren_depth == 0 && bracket_depth == 0 => {
                declarations.push(&body[start..=index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if !body[start..].trim().is_empty() {
        declarations.push(&body[start..]);
    }
    declarations
}

fn resolve_css_placeholders(
    body: &str,
    utility_name: &str,
    name: &str,
    modifier: Option<&Value>,
    theme: &ThemeIndex,
) -> Option<String> {
    let capture = utility_name
        .strip_suffix('*')
        .and_then(|prefix| name.strip_prefix(prefix));
    let mut body = body.to_owned();
    resolve_literal_value_placeholders(&mut body, capture, utility_name.starts_with('-'), theme)?;
    resolve_text_line_height_placeholders(&mut body, capture, theme)?;
    resolve_value_map_placeholders(&mut body, capture)?;
    while let Some(start) = body.find("--value(") {
        let open = start + "--value".len();
        let end = closing_parenthesis(&body, open)?;
        let domains = &body[open + 1..end];
        // The candidate grammar deliberately represents the slash in `w-1/2`
        // as a modifier. Value domains that explicitly opt into fractions can
        // reassemble that spelling without changing slash semantics for color
        // opacity, fonts, or arbitrary modifiers.
        let fraction_capture = if domains.contains("--fraction()") {
            capture.zip(modifier).and_then(|(capture, modifier)| {
                let modifier = match modifier {
                    Value::Named(value) => value,
                    Value::Arbitrary(_) | Value::CssVariable(_) => return None,
                };
                (capture.parse::<u16>().is_ok() && modifier.parse::<u16>().is_ok())
                    .then(|| format!("{capture}/{modifier}"))
            })
        } else {
            None
        };
        let capture = fraction_capture.as_deref().or(capture);
        let replacement =
            resolve_value_domains(domains, capture, utility_name.starts_with('-'), theme)?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier(") {
        let open = start + "--modifier".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let domains = &body[open + 1..end];
        let replacement = if domains.trim().is_empty() {
            match modifier {
                Value::Named(value) | Value::Arbitrary(value) => value.clone(),
                Value::CssVariable(value) => format!("var({value})"),
            }
        } else {
            let capture = match modifier {
                Value::Named(value) => value.clone(),
                // `Value::Arbitrary` is decoded by the grammar. Re-wrap it
                // so the same typed-domain resolver used by `--value(...)`
                // can check an optional arbitrary-value hint.
                Value::Arbitrary(value) => format!("[{value}]"),
                Value::CssVariable(value) => format!("({value})"),
            };
            resolve_value_domains(domains, Some(&capture), false, theme)?
        };
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-map(") {
        let open = start + "--modifier-map".len();
        let end = closing_parenthesis(&body, open)?;
        let modifier = modifier?;
        let modifier = match modifier {
            Value::Named(value) | Value::Arbitrary(value) => value,
            Value::CssVariable(_) => return None,
        };
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == modifier).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--modifier-none()") {
        if modifier.is_some() {
            return None;
        }
        let declaration_start = body[..start].rfind(';').map_or(0, |end| end + 1);
        let declaration_end = body[start..].find(';').map_or_else(
            || start + "--modifier-none()".len(),
            |offset| start + offset + 1,
        );
        body.replace_range(declaration_start..declaration_end, "");
    }
    while let Some(start) = body.find("--alpha(") {
        let open = start + "--alpha".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = alpha(&body[open + 1..end])?;
        body.replace_range(start..=end, &replacement);
    }
    while let Some(start) = body.find("--shadow-color(") {
        let open = start + "--shadow-color".len();
        let end = closing_parenthesis(&body, open)?;
        let replacement = resolve_shadow_color(&body[open + 1..end], capture, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    (!body.contains("--value(")
        && !body.contains("--value-literal(")
        && !body.contains("--text-line-height()")
        && !body.contains("--value-map(")
        && !body.contains("--modifier(")
        && !body.contains("--modifier-map(")
        && !body.contains("--modifier-none()")
        && !body.contains("--alpha(")
        && !body.contains("--shadow-color("))
    .then_some(body)
}

/// CSS-first extension for theme shadow opacity modifiers. `--value()` emits
/// the normal `var(--shadow-*)` reference; this intentionally reads the
/// authored token to construct a CSS Color 5 relative colour function.
fn resolve_shadow_color(
    arguments: &str,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<String> {
    let arguments = split_domains(arguments);
    let [domain, color_property, alpha] = arguments.as_slice() else {
        return None;
    };
    let namespace = domain
        .trim()
        .strip_prefix("--")
        .and_then(|domain| domain.strip_suffix("-*"))
        .and_then(ThemeNamespace::parse)?;
    let value = theme_value_literal(namespace, capture?, false, theme)?;
    let color_property = color_property.trim();
    color_property.strip_prefix("--").filter(|name| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
    })?;
    shadow_with_color_fallback(&value, color_property, alpha.trim())
}

fn resolve_literal_value_placeholders(
    body: &mut String,
    capture: Option<&str>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<()> {
    while let Some(start) = body.find("--value-literal(") {
        let open = start + "--value-literal".len();
        let end = closing_parenthesis(body, open)?;
        let replacement =
            resolve_literal_value_domains(&body[open + 1..end], capture, negative, theme)?;
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

fn resolve_text_line_height_placeholders(
    body: &mut String,
    capture: Option<&str>,
    theme: &ThemeIndex,
) -> Option<()> {
    while let Some(start) = body.find("--text-line-height()") {
        let capture = capture?;
        let token = theme.token_in(ThemeNamespace::Text, &format!("{capture}--line-height"))?;
        body.replace_range(
            start..start + "--text-line-height()".len(),
            &format!("var({})", token.name.value),
        );
    }
    Some(())
}

fn resolve_value_map_placeholders(body: &mut String, capture: Option<&str>) -> Option<()> {
    while let Some(start) = body.find("--value-map(") {
        let open = start + "--value-map".len();
        let end = closing_parenthesis(body, open)?;
        let capture = capture?;
        let replacement = split_domains(&body[open + 1..end])
            .into_iter()
            .find_map(|entry| {
                let (key, value) = entry.trim().split_once(':')?;
                (key.trim() == capture).then(|| value.trim())
            })?
            .to_owned();
        body.replace_range(start..=end, &replacement);
    }
    Some(())
}

fn split_nested_conditions(body: &str) -> Option<(String, Vec<ParsedNestedUtilityRule>)> {
    let mut direct = String::with_capacity(body.len());
    let mut nested = Vec::new();
    let mut cursor = 0;
    while let Some((start, at_rule, keyword)) = next_nested_condition(body, cursor) {
        direct.push_str(&body[cursor..start]);
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        let condition = body[start + keyword.len()..open].trim();
        let condition = match at_rule {
            NestedUtilityAtRule::Media => condition.to_owned(),
            NestedUtilityAtRule::Supports => condition
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))?
                .trim()
                .to_owned(),
        };
        nested.push((at_rule, condition, body[open + 1..close].trim().to_owned()));
        cursor = close + 1;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_owned(), nested))
}

fn next_nested_condition(
    body: &str,
    cursor: usize,
) -> Option<(usize, NestedUtilityAtRule, &'static str)> {
    let supports = body[cursor..]
        .find("@supports")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Supports, "@supports"));
    let media = body[cursor..]
        .find("@media")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Media, "@media"));
    match (supports, media) {
        (Some(supports), Some(media)) => Some(if supports.0 < media.0 {
            supports
        } else {
            media
        }),
        (Some(condition), None) | (None, Some(condition)) => Some(condition),
        (None, None) => None,
    }
}

fn split_utility_selector(body: &str) -> Option<(Option<String>, &str)> {
    let Some(selector_start) = body.find('&') else {
        return Some((None, body));
    };
    let open = body[selector_start..].find('{')? + selector_start;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[selector_start..open].trim();
    (!selector.is_empty()).then(|| (Some(selector.to_owned()), body[open + 1..close].trim()))
}

fn closing_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_utilities<'a>(
    utilities: &'a [CustomUtility],
    name: &'a str,
) -> impl Iterator<Item = &'a CustomUtility> {
    utilities.iter().rev().filter(move |utility| {
        utility.name == name
            || utility
                .name
                .strip_suffix('*')
                .is_some_and(|prefix| name.starts_with(prefix))
    })
}

fn closing_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve_value_domains(
    domains: &str,
    capture: Option<&str>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        if domain == "integer" {
            return capture
                .filter(|value| value.parse::<i64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "number" {
            return capture
                .filter(|value| value.parse::<f64>().is_ok())
                .map(str::to_owned);
        }
        if domain == "percentage" {
            return capture
                .and_then(|value| value.strip_suffix('%'))
                .filter(|value| value.parse::<f64>().is_ok())
                .map(|value| format!("{value}%"));
        }
        if domain == "--spacing()" {
            return capture.and_then(|capture| {
                theme_value(ThemeNamespace::Spacing, capture, negative, theme)
            });
        }
        if domain == "--fraction()" {
            return capture.and_then(|capture| fraction_value(capture, negative));
        }
        if domain == "--inset()" {
            return capture.and_then(|capture| inset_value(capture, negative));
        }
        if domain == "--width()" {
            return capture.and_then(|capture| axis_size_value(capture, "vw", negative));
        }
        if domain == "--height()" {
            return capture.and_then(|capture| axis_size_value(capture, "vh", negative));
        }
        if let Some(namespace) = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse)
        {
            return capture.and_then(|capture| theme_value(namespace, capture, negative, theme));
        }
        if !domain.starts_with('[') && !domain.starts_with("--") {
            return capture.filter(|value| *value == domain).map(str::to_owned);
        }
        let expected = ValueDomain::from_hint(domain.strip_prefix('[')?.strip_suffix(']')?)?;
        let arbitrary = capture?.strip_prefix('[')?.strip_suffix(']')?;
        let (hint, value) = arbitrary.split_once(':').unwrap_or(("", arbitrary));
        let value = decode_arbitrary(value);
        let hint_is_compatible = hint.is_empty() || expected.accepts_hint(hint);
        (hint_is_compatible && expected.accepts(&value)).then_some(value)
    })
}

/// Resolves a typed domain to its authored theme token value rather than a
/// `var(--token)` reference. Tailwind uses this form for broad-gamut-safe sRGB
/// fallbacks, while retaining the variable reference in the upgraded branch.
fn resolve_literal_value_domains(
    domains: &str,
    capture: Option<&str>,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    split_domains(domains).into_iter().find_map(|domain| {
        let domain = domain.trim();
        let namespace = domain
            .strip_prefix("--")
            .and_then(|domain| domain.strip_suffix("-*"))
            .and_then(ThemeNamespace::parse);
        if let Some(namespace) = namespace {
            return capture
                .and_then(|capture| theme_value_literal(namespace, capture, negative, theme));
        }
        resolve_value_domains(domain, capture, negative, theme)
    })
}

fn split_domains(value: &str) -> Vec<&str> {
    let mut domains = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (index, character) in value.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                domains.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    domains.push(&value[start..]);
    domains
}

fn theme_value(
    namespace: ThemeNamespace,
    value: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        // These are CSS colour keywords, not `@theme` tokens. They are
        // nevertheless part of every Tailwind colour utility's public value
        // grammar, including after an application resets `--color-*`.
        (ThemeNamespace::Color, "current") => "currentcolor".to_owned(),
        (ThemeNamespace::Color, "inherit" | "transparent") => value.to_owned(),
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = css_variable(value) => format!("var({value})"),
        // Tailwind's candidate grammar accepts ASCII identifier tokens only;
        // Unicode theme names remain valid CSS but are not utility candidates.
        _ if value.chars().any(|character| !character.is_ascii()) => return None,
        _ if let Some(token) = theme.token_in(namespace, value) => {
            if matches!(token.mode, stillwind_css::ThemeTokenMode::Inline) {
                token.value.value.clone()
            } else {
                let prefix = theme
                    .imports
                    .prefix
                    .as_deref()
                    .map(|prefix| format!("{}-", prefix.trim_end_matches('-')))
                    .unwrap_or_default();
                format!("var(--{prefix}{}-{value})", namespace.css_name())
            }
        }
        _ if namespace == ThemeNamespace::Spacing
            && theme.scale(ThemeNamespace::Spacing).is_some() =>
        {
            if value == "px" {
                "1px".to_owned()
            } else if value.parse::<f64>().is_ok() {
                if value.parse::<f64>().is_ok_and(|number| number == 0.0) {
                    return Some("0".to_owned());
                }
                let multiplier = if negative {
                    format!("-{value}")
                } else {
                    value.to_owned()
                };
                return Some(format!("calc(var(--spacing) * {multiplier})"));
            } else {
                return None;
            }
        }
        _ => return None,
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn theme_value_literal(
    namespace: ThemeNamespace,
    value: &str,
    negative: bool,
    theme: &ThemeIndex,
) -> Option<String> {
    let value = match (namespace, value) {
        (ThemeNamespace::Color, "current") => "currentcolor".to_owned(),
        (ThemeNamespace::Color, "inherit" | "transparent") => value.to_owned(),
        _ if let Some(value) = typed_theme_arbitrary(namespace, value) => value,
        _ if let Some(value) = css_variable(value) => format!("var({value})"),
        _ if let Some(token) = theme.token_in(namespace, value) => token.value.value.clone(),
        _ => return theme_value(namespace, value, negative, theme),
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn typed_theme_arbitrary(namespace: ThemeNamespace, value: &str) -> Option<String> {
    let value = value.strip_prefix('[')?.strip_suffix(']')?;
    let (hint, value) = value.split_once(':').unwrap_or(("", value));
    let value = decode_arbitrary(value);
    let types = theme_arbitrary_types(namespace);
    let hint_is_compatible = hint.is_empty()
        || types.is_empty()
        || ValueType::from_hint(hint)
            .is_some_and(|hint| types.iter().any(|expected| expected.accepts_hint(hint)));
    (hint_is_compatible && (types.is_empty() || types.iter().any(|kind| kind.accepts(&value))))
        .then_some(value)
}

fn theme_arbitrary_types(namespace: ThemeNamespace) -> &'static [ValueType] {
    match namespace {
        ThemeNamespace::Color => &[ValueType::Color],
        ThemeNamespace::Spacing
        | ThemeNamespace::Radius
        | ThemeNamespace::Blur
        | ThemeNamespace::Breakpoint
        | ThemeNamespace::Container
        | ThemeNamespace::Perspective
        | ThemeNamespace::Tracking => &[ValueType::Length],
        ThemeNamespace::AspectRatio => &[ValueType::Ratio],
        ThemeNamespace::Animate => &[ValueType::Animation],
        ThemeNamespace::DropShadow
        | ThemeNamespace::InsetShadow
        | ThemeNamespace::Shadow
        | ThemeNamespace::TextShadow => &[ValueType::Shadow],
        ThemeNamespace::Ease => &[ValueType::Easing],
        ThemeNamespace::Font => &[ValueType::FontFamily],
        ThemeNamespace::FontWeight => &[ValueType::FontWeight],
        ThemeNamespace::Leading => &[ValueType::Number, ValueType::Length],
        ThemeNamespace::FontStretch => &[ValueType::Percentage],
        ThemeNamespace::Text => &[ValueType::FontSize],
        ThemeNamespace::Duration => &[ValueType::Time],
        ThemeNamespace::ZIndex => &[ValueType::ZIndex],
    }
}

fn fraction_value(value: &str, negative: bool) -> Option<String> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u16>().ok()?;
    let denominator = denominator.parse::<u16>().ok()?;
    (denominator != 0).then(|| {
        let value = format!("calc({numerator} / {denominator} * 100%)");
        if negative {
            format!("calc({value} * -1)")
        } else {
            value
        }
    })
}

fn inset_value(value: &str, negative: bool) -> Option<String> {
    let value = match value {
        "auto" if !negative => "auto".to_owned(),
        "full" => "100%".to_owned(),
        _ => return None,
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
}

fn axis_size_value(value: &str, viewport_axis: &str, negative: bool) -> Option<String> {
    let value = match value {
        "auto" => "auto".to_owned(),
        "full" => "100%".to_owned(),
        "screen" => format!("100{viewport_axis}"),
        "svw" => "100svw".to_owned(),
        "lvw" => "100lvw".to_owned(),
        "dvw" => "100dvw".to_owned(),
        "svh" => "100svh".to_owned(),
        "lvh" => "100lvh".to_owned(),
        "dvh" => "100dvh".to_owned(),
        "min" => "min-content".to_owned(),
        "max" => "max-content".to_owned(),
        "fit" => "fit-content".to_owned(),
        "lh" => "1lh".to_owned(),
        _ => return None,
    };
    negative
        .then(|| format!("calc({value} * -1)"))
        .or(Some(value))
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

fn alpha(value: &str) -> Option<String> {
    if value.starts_with("var(") {
        return Some(value.to_owned());
    }
    let number = value.parse::<f64>().ok()?;
    Some(if number <= 1.0 {
        format!("{}%", number * 100.0)
    } else {
        format!("{number}%")
    })
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
    use std::collections::BTreeSet;

    use stillwind_css::parse_tailwind_theme;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    const SIMPLE_CSSWG_TERMS: &[&str] = &[
        "angle",
        "angle-percentage",
        "color",
        "easing-function",
        "image",
        "integer",
        "length",
        "length-percentage",
        "line-width",
        "number",
        "number-percentage",
        "percentage",
        "position",
        "ratio",
        "time",
        "url",
    ];

    fn property_references(syntax: &str) -> Vec<&str> {
        let mut references = Vec::new();
        let mut remainder = syntax;
        while let Some(start) = remainder.find("<'") {
            let after_start = &remainder[start + 2..];
            let Some(end) = after_start.find("'>") else {
                break;
            };
            references.push(&after_start[..end]);
            remainder = &after_start[end + 2..];
        }
        references
    }

    fn expanded_csswg_syntax(property: &str, visited: &mut BTreeSet<String>) -> Option<String> {
        if !visited.insert(property.to_owned()) {
            return Some(String::new());
        }
        let syntax = stillwind_css::csswg_formal_syntax(property)?;
        let mut expanded = syntax.to_owned();
        for referenced_property in property_references(syntax) {
            expanded.push(' ');
            expanded.push_str(&expanded_csswg_syntax(referenced_property, visited)?);
        }
        Some(expanded)
    }

    fn formal_terms(syntax: &str) -> BTreeSet<&str> {
        let mut terms = BTreeSet::new();
        let mut remainder = syntax;
        while let Some(start) = remainder.find('<') {
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find('>') else {
                break;
            };
            if after_start.starts_with('\'') {
                remainder = &after_start[end + 1..];
                continue;
            }
            let term = after_start[..end]
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
                .next()
                .unwrap_or_default();
            if !term.is_empty() {
                terms.insert(term);
            }
            remainder = &after_start[end + 1..];
        }
        terms
    }

    fn direct_value_domains(value: &str) -> Vec<ValueType> {
        if !value.trim_start().starts_with("--value(") {
            return Vec::new();
        }
        let mut domains = Vec::new();
        let mut remainder = value;
        while let Some(start) = remainder.find('[') {
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find(']') else {
                break;
            };
            if let Some(domain) = ValueType::from_hint(&after_start[..end]) {
                domains.push(domain);
            }
            remainder = &after_start[end + 1..];
        }
        domains
    }

    fn has_complex_csswg_grammar(syntax: &str, terms: &BTreeSet<&str>) -> bool {
        syntax.contains("&&")
            || syntax.contains("||")
            || terms.iter().any(|term| !SIMPLE_CSSWG_TERMS.contains(term))
    }

    fn theme() -> ThemeIndex {
        parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-brand: red; }",
        ))
        .unwrap()
    }

    #[test]
    fn builtins_declare_no_untyped_arbitrary_value_domains() {
        assert!(
            !BUILTIN_REGISTRY_CSS.contains("[*]"),
            "built-in utilities must use a typed CSS value domain"
        );
        assert!(
            !BUILTIN_REGISTRY_CSS.contains("[property:"),
            "property names are not CSS value-domain syntax"
        );
    }

    #[test]
    fn simple_utility_domains_follow_csswg_formal_syntax() {
        let mut checked = BTreeSet::new();
        let mut failures = Vec::new();
        for utility in stillwind_css::parse_utility_definitions(BUILTIN_REGISTRY_CSS) {
            for declaration in utility.body.split(';') {
                let Some((property, value)) = declaration.split_once(':') else {
                    continue;
                };
                let property = property.trim();
                if property.starts_with('-') || property.contains('{') {
                    continue;
                }
                for domain in direct_value_domains(value) {
                    let Some(expected_terms) = domain.csswg_formal_terms() else {
                        continue;
                    };
                    let mut visited = BTreeSet::new();
                    let Some(syntax) = expanded_csswg_syntax(property, &mut visited) else {
                        failures.push(format!(
                            "{property} in @utility {} has no Webref formal syntax",
                            utility.name
                        ));
                        continue;
                    };
                    let terms = formal_terms(&syntax);
                    // A named production such as `<grid-line>` needs
                    // property-specific interpretation. Those compound
                    // grammars are detected from their syntax rather than a
                    // per-property exception list.
                    if has_complex_csswg_grammar(&syntax, &terms) {
                        continue;
                    }
                    if expected_terms
                        .iter()
                        .any(|expected| terms.contains(expected))
                    {
                        checked.insert(format!("{property} [{domain:?}]"));
                    } else {
                        failures.push(format!(
                            "{property} in @utility {} uses [{domain:?}], but CSSWG declares {syntax:?}",
                            utility.name
                        ));
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "built-in simple value domains diverge from CSSWG formal syntax:\n{}",
            failures.join("\n")
        );
        assert!(
            checked.len() >= 20,
            "the registry-wide audit unexpectedly covered only {} declarations: {checked:?}",
            checked.len()
        );
    }

    #[test]
    fn every_tailwind_theme_namespace_has_a_typed_arbitrary_value_domain() {
        for namespace in [
            ThemeNamespace::Animate,
            ThemeNamespace::AspectRatio,
            ThemeNamespace::Blur,
            ThemeNamespace::Breakpoint,
            ThemeNamespace::Color,
            ThemeNamespace::Container,
            ThemeNamespace::DropShadow,
            ThemeNamespace::Ease,
            ThemeNamespace::Font,
            ThemeNamespace::FontStretch,
            ThemeNamespace::FontWeight,
            ThemeNamespace::InsetShadow,
            ThemeNamespace::Leading,
            ThemeNamespace::Perspective,
            ThemeNamespace::Radius,
            ThemeNamespace::Shadow,
            ThemeNamespace::Spacing,
            ThemeNamespace::Text,
            ThemeNamespace::TextShadow,
            ThemeNamespace::Tracking,
            ThemeNamespace::ZIndex,
            ThemeNamespace::Duration,
        ] {
            assert!(
                !theme_arbitrary_types(namespace).is_empty(),
                "{namespace:?} must not fall back to an unchecked arbitrary value"
            );
        }
    }

    fn catalog_section<'a>(markdown: &'a str, heading: &str) -> Vec<&'a str> {
        let start = markdown
            .find(heading)
            .and_then(|start| {
                markdown[start..]
                    .find("```text\n")
                    .map(|offset| start + offset + 8)
            })
            .expect("catalog section must contain candidates");
        markdown[start..]
            .split_once("\n```")
            .expect("catalog section must close")
            .0
            .split_whitespace()
            .collect()
    }

    #[test]
    fn resolves_every_upstream_static_utility_from_the_css_registry() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let candidates = catalog_section(catalog, "## Static utilities");
        let theme = theme();

        let unresolved = candidates
            .into_iter()
            .filter(|candidate| resolve_utility(candidate, &theme).is_none())
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved static utilities: {unresolved:?}"
        );
    }

    #[test]
    fn resolves_every_upstream_functional_root_with_an_arbitrary_value() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let roots = catalog_section(catalog, "## Functional utility roots");
        let theme = theme();

        let unresolved = roots
            .into_iter()
            .map(|root| format!("{root}-[initial]"))
            .filter(|candidate| resolve_utility(candidate, &theme).is_none())
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved functional utility candidates: {unresolved:?}"
        );
    }

    #[test]
    fn resolves_every_upstream_static_variant_from_the_css_registry() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
        let variants = catalog_section(catalog, "## Static variants");
        let theme = theme();

        let unresolved = variants
            .into_iter()
            .filter(|name| {
                let candidate =
                    parse_candidate(&format!("{name}:flex")).expect("catalog variants must parse");
                resolve_variant(&candidate.variants[0], &theme).is_none()
            })
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "unresolved static variants: {unresolved:?}"
        );
    }

    #[test]
    fn preserves_a_nested_selector_from_a_css_utility() {
        let resolved = resolve_utility("divide-x-[2px]", &theme()).unwrap();

        assert_eq!(
            resolved.declaration,
            "border-inline-end-style:var(--tw-border-style);border-inline-end-width:2px;"
        );
        assert_eq!(resolved.selector.as_deref(), Some("&>:not(:last-child)"));
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
        let background = resolve_utility("bg-brand/50", &theme).unwrap();
        assert_eq!(
            background.declaration,
            "background-color:color-mix(in srgb, red 50%, transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.declaration
                    == "background-color:color-mix(in oklab, var(--color-brand) 50%, transparent);"
        }));
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
    fn respects_tailwind_theme_namespace_resets_when_resolving_utilities() {
        let default_theme = theme();
        assert_eq!(
            resolve_utility("bg-red-500", &default_theme)
                .expect("the vendored Tailwind theme provides red-500")
                .declaration,
            "background-color:var(--color-red-500);"
        );

        let reset_theme = parse_tailwind_theme(&SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-*: initial; --color-brand: rebeccapurple; }",
        ))
        .unwrap();
        assert!(resolve_utility("bg-red-500", &reset_theme).is_none());
        assert_eq!(
            resolve_utility("bg-brand", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:var(--color-brand);"
        );
        assert_eq!(
            resolve_utility("bg-current", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:currentcolor;"
        );
        assert_eq!(
            resolve_utility("bg-transparent", &reset_theme)
                .unwrap()
                .declaration,
            "background-color:transparent;"
        );
    }

    #[test]
    fn resolves_native_mask_conic_angles() {
        let resolved = resolve_utility("mask-conic-45", &theme()).expect("mask conic angle");
        assert!(
            resolved
                .declaration
                .contains("--tw-mask-conic-position:calc(1deg * 45);"),
            "{resolved:?}"
        );
        assert!(
            resolve_utility("-mask-conic-45", &theme())
                .expect("negative mask conic angle")
                .declaration
                .contains("--tw-mask-conic-position:calc(1deg * -45);")
        );
    }

    #[test]
    fn resolves_gradient_interpolation_modifiers_in_the_css_registry() {
        let default = resolve_utility("bg-conic-45", &theme()).expect("default conic");
        assert!(
            default
                .declaration
                .contains("--tw-gradient-position:from 45deg in oklab;"),
            "{default:?}"
        );
        let interpolated =
            resolve_utility("bg-conic-45/increasing", &theme()).expect("interpolated conic");
        assert!(interpolated
            .declaration
            .contains("from 45deg in oklch increasing hue"));
    }

    #[test]
    fn resolves_css_defined_color_and_gradient_utilities() {
        let theme = theme();
        assert_eq!(
            resolve_utility("fill-brand/50", &theme)
                .unwrap()
                .declaration,
            "fill:color-mix(in srgb, var(--color-brand) 50%, transparent);"
        );
        assert_eq!(
            resolve_utility("from-brand", &theme).unwrap().declaration,
            "--tw-gradient-from:var(--color-brand);--tw-gradient-stops:var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));"
        );
        assert_eq!(
            resolve_utility("bg-linear-to-r", &theme)
                .unwrap()
                .declaration,
            "--tw-gradient-position:to right;background-image:linear-gradient(var(--tw-gradient-stops));"
        );
    }

    #[test]
    fn resolves_ambiguous_backgrounds_through_typed_utility_branches() {
        let theme = theme();
        assert_eq!(
            resolve_utility("bg-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "background-color:rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("bg-[image:linear-gradient(red,blue)]", &theme)
                .unwrap()
                .declaration,
            "background-image:linear-gradient(red,blue);"
        );
        assert_eq!(
            resolve_utility("bg-[position:center_20%]", &theme)
                .unwrap()
                .declaration,
            "background-position:center 20%;"
        );
        assert_eq!(
            resolve_utility("bg-[bg-size:24rem_auto]", &theme)
                .unwrap()
                .declaration,
            "background-size:24rem auto;"
        );
    }

    #[test]
    fn prunes_only_incompatible_declarations_from_a_typed_utility_body() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility token-* {
  color: --value([color]);
  width: --value([length]);
}",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("token-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color: rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("token-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "width: 12px;"
        );
    }

    #[test]
    fn rejects_untyped_or_invalid_modifiers_instead_of_ignoring_them() {
        let theme = theme();

        assert!(resolve_utility("bg-red-500/not-an-opacity", &theme).is_none());
        assert!(resolve_utility("bg-[image:url(/hero.svg)]/50", &theme).is_none());
        let background = resolve_utility("bg-red-500/50", &theme).unwrap();
        assert_eq!(
            background.declaration,
            "background-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
        );
        assert!(background.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.declaration
                    == "background-color:color-mix(in oklab, var(--color-red-500) 50%, transparent);"
        }));
    }

    #[test]
    fn resolves_typed_modifier_domains_without_accepting_other_types() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility prose-* {
  font-size: --value(--text-*);
  line-height: --modifier(--leading-*, --spacing(), [length]);
}",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("prose-sm/6", &theme).unwrap().declaration,
            "font-size: var(--text-sm);\n  line-height: calc(var(--spacing) * 6);"
        );
        assert!(resolve_utility("prose-sm/[color:rebeccapurple]", &theme).is_none());
    }

    #[test]
    fn preserves_supports_and_media_conditions_from_css_utilities() {
        let theme = theme();
        let outline = resolve_utility("outline-hidden", &theme).unwrap();

        assert!(outline.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Media
                && rule.condition == "(forced-colors:active)"
                && rule.declaration == "outline:2px solid transparent;outline-offset:2px;"
        }));
    }

    #[test]
    fn resolves_tailwind_relationship_compatibility_primitives() {
        let theme = theme();

        for candidate in [
            "container",
            "@container",
            "@container-normal",
            "group",
            "peer",
            "group-hover:flex",
            "peer-checked:flex",
            "@sm:flex",
        ] {
            assert!(is_tailwind_relationship_candidate(candidate), "{candidate}");
        }
        assert!(!is_tailwind_relationship_candidate("max-w-lg"));
        assert_eq!(
            compatibility_marker_class("group").as_deref(),
            Some("group")
        );
        assert_eq!(
            compatibility_marker_class("peer/field").as_deref(),
            Some("peer/field")
        );
        let container = resolve_utility("container", &theme).unwrap();
        assert_eq!(container.declaration, "width:100%;");
        assert!(container.nested.iter().any(|rule| {
            rule.condition == "(width >= 48rem)" && rule.declaration == "max-width:48rem;"
        }));
        assert_eq!(
            resolve_utility("@container", &theme).unwrap().declaration,
            "container-type:inline-size;"
        );
        assert_eq!(
            resolve_utility("@container/sidebar", &theme)
                .unwrap()
                .declaration,
            "container-name:sidebar;container-type:inline-size;"
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("group-hover:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorMedia {
                selector: "&:is(:where(.group):hover *)".to_owned(),
                condition: "(hover: hover)".to_owned(),
            })
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("peer-checked:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&:is(:where(.peer):checked ~ *)".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("group-[&[data-open]]:flex")
                    .unwrap()
                    .variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&:is(:where(.group)[data-open] *)".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
    }

    #[test]
    fn resolves_numeric_css_domains_and_default_filter_tokens() {
        let theme = theme();
        let custom_theme = ThemeIndex {
            utilities: stillwind_css::parse_utility_definitions(
                "@utility tab-* { tab-size: --value(integer); }",
            ),
            ..ThemeIndex::default()
        };

        assert_eq!(
            resolve_utility("tab-8", &custom_theme).unwrap().declaration,
            "tab-size: 8;"
        );
        assert_eq!(
            resolve_utility("blur-sm", &theme).unwrap().declaration,
            "--tw-blur:blur(var(--blur-sm));filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
        );
    }

    #[test]
    fn resolves_theme_shadow_opacity_with_a_relative_colour_function() {
        let shadow = resolve_utility("inset-shadow-sm/50", &theme()).unwrap();

        assert!(shadow.declaration.contains("--tw-inset-shadow-alpha:50%;"));
        assert!(
            shadow.declaration.contains(
                "--tw-inset-shadow:inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%));"
            ),
            "{}",
            shadow.declaration
        );
        assert!(shadow.declaration.contains(
            "box-shadow:var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);"
        ));
        assert!(resolve_utility("inset-shadow-sm/not-an-opacity", &theme()).is_none());
    }

    #[test]
    fn resolves_css_wide_keywords_for_property_typed_built_ins() {
        for candidate in [
            "contain-[initial]",
            "font-features-[initial]",
            "object-[initial]",
            "will-change-[initial]",
            "zoom-[initial]",
        ] {
            assert!(
                resolve_utility(candidate, &theme()).is_some(),
                "{candidate} must resolve through its exact CSS property grammar"
            );
        }
    }

    #[test]
    fn resolves_compact_spacing_families_with_negative_and_fractional_values() {
        let theme = theme();

        assert_eq!(
            resolve_utility("-left-4", &theme).unwrap().declaration,
            "left:calc(var(--spacing) * -4);"
        );
        assert_eq!(
            resolve_utility("scroll-px-4", &theme).unwrap().declaration,
            "scroll-padding-inline:calc(var(--spacing) * 4);"
        );
        assert_eq!(
            resolve_utility("-left-1/2", &theme).unwrap().declaration,
            "left:calc(calc(1 / 2 * 100%) * -1);"
        );
        assert_eq!(
            resolve_utility("-bottom-full", &theme).unwrap().declaration,
            "bottom:calc(100% * -1);"
        );
    }

    #[test]
    fn resolves_state_attribute_responsive_and_selector_variants() {
        let theme = theme();
        assert_eq!(
            resolve_variant(&parse_candidate("hover:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::SelectorMedia {
                selector: "&:hover".to_owned(),
                condition: "(hover: hover)".to_owned(),
            })
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
        assert_eq!(
            resolve_variant(
                &parse_candidate("motion-reduce:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::Media(
                "(prefers-reduced-motion: reduce)".to_owned()
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
            Some(VariantApplication::Media("(width >= 40rem)".to_owned()))
        );
        assert_eq!(
            resolve("@sm:flex"),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
        assert_eq!(
            resolve("nth-[3n+1]:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:nth-child(3n+1)".to_owned()
            ))
        );
        assert_eq!(
            resolve("aria-checked:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-checked=\"true\"]".to_owned()
            ))
        );
        assert_eq!(
            resolve("has-checked:flex"),
            Some(VariantApplication::SelectorTemplate(
                "&:has(:checked)".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_user_utilities_with_the_same_value_domains_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@theme { --color-brand: red; }
@utility tone-* { color: --value(--color-*); }
@utility rail-* { inline-size: --value(--spacing()); }
@utility mask-* { mask-size: --value([length]); }
@utility alpha-* { opacity: --modifier(); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("tone-brand", &theme).unwrap().declaration,
            "color: var(--color-brand);"
        );
        assert_eq!(
            resolve_utility("rail-4", &theme).unwrap().declaration,
            "inline-size: calc(var(--spacing) * 4);"
        );
        assert_eq!(
            resolve_utility("mask-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "mask-size: 12px;"
        );
        assert_eq!(
            resolve_utility("alpha-brand/50", &theme)
                .unwrap()
                .declaration,
            "opacity: 50;"
        );
    }

    #[test]
    fn tries_overloaded_css_utility_definitions_in_reverse_source_order() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r"@utility token-* { width: --value([length]); }
@utility token-* { color: --value([color]); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("token-[color:rebeccapurple]", &theme)
                .unwrap()
                .declaration,
            "color: rebeccapurple;"
        );
        assert_eq!(
            resolve_utility("token-[length:12px]", &theme)
                .unwrap()
                .declaration,
            "width: 12px;"
        );
    }

    #[test]
    fn preserves_css_fallbacks_and_multiple_selector_targets() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            r#"@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
        );
        let theme = parse_tailwind_theme(&source).unwrap();
        let resolved = resolve_utility("paint-card", &theme).unwrap();

        assert!(
            resolved
                .declaration
                .contains("color: red; color: color(display-p3 1 0 0);")
                || resolved.additional.iter().any(|target| target
                    .declaration
                    .contains("color: red; color: color(display-p3 1 0 0);")),
            "{resolved:#?}"
        );
        assert_eq!(resolved.additional.len(), 1);
        assert_eq!(resolved.selector.as_deref(), Some("&::before"));
        assert_eq!(resolved.declaration, "content: \"fallback\";");
        assert_eq!(
            resolved.additional[0].declaration,
            "color: red; color: color(display-p3 1 0 0);"
        );
    }

    #[test]
    fn color_alpha_family_emits_srgb_fallback_and_oklab_upgrade() {
        let resolved = resolve_utility("text-brand/50", &theme()).unwrap();
        assert_eq!(
            resolved.declaration,
            "color:color-mix(in srgb, var(--color-brand) 50%, transparent);"
        );
        assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration
                    == "color:color-mix(in oklab, var(--color-brand) 50%, transparent);"
        }));
        assert!(resolve_utility("text-brand/[length:12px]", &theme()).is_none());
    }

    #[test]
    fn shadow_color_alpha_emits_hex_fallback_and_upgrade() {
        let resolved = resolve_utility("shadow-red-500/50", &theme()).unwrap();
        assert!(
            resolved
                .declaration
                .contains("--tw-shadow-color:#fb2c3680;")
        );
        assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration.contains(
                    "--tw-shadow-color:color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);",
                )
        }));
    }

    #[test]
    fn resolves_tailwind_static_keyword_utilities_from_the_css_registry() {
        let theme = theme();
        for (candidate, declaration) in [
            ("cursor-alias", "cursor:alias;"),
            ("overflow-scroll", "overflow:scroll;"),
            ("overscroll-y-none", "overscroll-behavior-y:none;"),
            ("break-after-page", "break-after:page;"),
            ("z-auto", "z-index:auto;"),
            ("z-73", "z-index:73;"),
            ("-z-73", "z-index:calc(73*-1);"),
            ("border-13", "border-width:calc(13*1px);"),
            ("col-span-full", "grid-column:1/-1;"),
            ("row-auto", "grid-row:auto;"),
            (
                "rounded-tl-full",
                "border-top-left-radius:calc(infinity*1px);",
            ),
            ("touch-pan-left", "--tw-pan-x:pan-left;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
        }
    }

    #[test]
    fn resolves_discrete_tailwind_keywords_without_fixture_mappings() {
        let theme = theme();
        for (candidate, declaration, selector) in [
            (
                "divide-y",
                "--tw-divide-y-reverse:0;border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * calc(1 - var(--tw-divide-y-reverse)));",
                Some("&>:not(:last-child)"),
            ),
            (
                "divide-dashed",
                "border-style:dashed;",
                Some("&>:not(:last-child)"),
            ),
            ("m-auto", "margin:auto;", None),
            ("mix-blend-color-dodge", "mix-blend-mode:color-dodge;", None),
            ("object-bottom-left", "object-position:left bottom;", None),
            ("outline", "outline-style:solid;outline-width:1px;", None),
            ("ring", "--tw-ring-shadow:0 0 0 1px", None),
            (
                "rounded-se-full",
                "border-start-end-radius:calc(infinity*1px);",
                None,
            ),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
            assert_eq!(resolved.selector.as_deref(), selector, "{candidate}");
        }
    }

    #[test]
    fn resolves_fraction_and_theme_value_families_from_typed_css_domains() {
        let theme = theme();
        for (candidate, declaration) in [
            ("flex-5/12", "flex:calc(5 / 12 * 100%);"),
            ("columns-lg", "columns:var(--container-lg);"),
            ("font-stretch-125%", "font-stretch:125%;"),
            ("bg-radial", "--tw-gradient-position:in oklab;"),
            ("bg-conic", "--tw-gradient-position:in oklab;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
        }
    }

    #[test]
    fn resolves_text_lengths_and_additional_discrete_layout_keywords() {
        let theme = theme();
        for (candidate, declaration) in [
            ("text-[length:1.25rem]", "font-size:1.25rem;"),
            ("grid-cols-subgrid", "grid-template-columns:subgrid;"),
            ("leading-none", "line-height:1;"),
            ("origin-bottom-left", "transform-origin:bottom left;"),
        ] {
            let resolved = resolve_utility(candidate, &theme)
                .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
            assert!(
                resolved.declaration.contains(declaration),
                "{candidate}: {resolved:#?}"
            );
        }

        let custom_theme = parse_tailwind_theme(&SourceFile::new(
            FileId(2),
            "theme.css",
            "@theme { --font-stretch-superbold: 150%; }",
        ))
        .unwrap();
        assert!(
            custom_theme
                .token_in(ThemeNamespace::FontStretch, "superbold")
                .is_some()
        );
        assert_eq!(
            resolve_utility("font-stretch-superbold", &custom_theme)
                .unwrap()
                .declaration,
            "font-stretch:var(--font-stretch-superbold);"
        );
    }

    #[test]
    fn validates_unhinted_and_hint_checked_arbitrary_css_values() {
        let theme = theme();
        for candidate in [
            "p-[1rem]",
            "p-[50%]",
            "p-[calc(1rem_+_50%)]",
            "p-[length:1rem]",
            "p-[percentage:50%]",
            "border-[3px]",
            "border-[thin]",
            "z-[12]",
            "z-[auto]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_some(),
                "expected `{candidate}` to resolve"
            );
        }
        assert_eq!(
            resolve_utility("p-[50%]", &theme).unwrap().declaration,
            "padding:50%;"
        );
        for candidate in [
            "p-[length:bogus]",
            "border-[time:1ms]",
            "border-[length:bogus]",
            "border-[50%]",
            "z-[1.5]",
            "z-[integer:1.5]",
            "z-[number:1]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_none(),
                "expected `{candidate}` to be rejected"
            );
        }
    }

    #[test]
    fn validates_arbitrary_utilities_against_property_grammars() {
        let theme = theme();
        for candidate in [
            "opacity-[.5]",
            "delay-[150ms]",
            "aspect-[16/9]",
            "grid-cols-[repeat(2,_minmax(0,_1fr))]",
            "content-['stillwind']",
            "mask-linear-[45deg,_red,_blue]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_some(),
                "expected `{candidate}` to resolve"
            );
        }
        for candidate in [
            "opacity-[red]",
            "delay-[red]",
            "aspect-[red]",
            "grid-cols-[red]",
            "content-[red]",
            "mask-linear-[red_/_blue]",
        ] {
            assert!(
                resolve_utility(candidate, &theme).is_none(),
                "expected `{candidate}` to be rejected"
            );
        }
    }

    #[test]
    fn matches_tailwind_unbracketed_value_grammar_catalog() {
        let catalog = include_str!("../../../fixtures/tailwind-v4/catalog/value-grammar.md");
        let theme = theme();
        for (heading, expected) in [
            ("## Padding: accepted without brackets", true),
            ("## Padding: rejected without brackets", false),
            ("## Border width: accepted without brackets", true),
            ("## Border width: rejected without brackets", false),
            ("## Z index: accepted without brackets", true),
            ("## Z index: rejected without brackets", false),
        ] {
            for candidate in catalog_section(catalog, heading) {
                assert_eq!(
                    resolve_utility(candidate, &theme).is_some(),
                    expected,
                    "{heading}: `{candidate}`"
                );
            }
        }
    }

    #[test]
    fn resolves_placeholder_colours_as_typed_pseudo_element_utilities() {
        let resolved = resolve_utility("placeholder-current/50", &theme()).unwrap();
        assert_eq!(resolved.selector.as_deref(), Some("&::placeholder"));
        assert!(
            resolved
                .declaration
                .contains("color:color-mix(in oklab, currentcolor 50%, transparent);"),
            "{resolved:#?}"
        );
        assert!(resolve_utility("placeholder-[length:1rem]", &theme()).is_none());
    }

    #[test]
    fn resolves_gradient_direction_and_interpolation_from_css_maps() {
        let resolved = resolve_utility("bg-linear-to-r/oklch", &theme()).unwrap();
        assert!(
            resolved
                .declaration
                .contains("--tw-gradient-position:to right;")
        );
        assert!(resolved.nested.iter().any(|nested| nested.declaration.contains("to right in oklch")));
        let radial = resolve_utility("bg-radial/hsl", &theme()).unwrap();
        assert!(
            radial
                .declaration
                .contains("--tw-gradient-position:in hsl;")
        );
    }

    #[test]
    fn resolves_size_as_a_functional_fraction_or_keyword() {
        let fractional = resolve_utility("size-7/12", &theme()).unwrap();
        assert!(
            fractional
                .declaration
                .contains("width:calc(7 / 12 * 100%);height:calc(7 / 12 * 100%);")
        );
        assert_eq!(
            resolve_utility("size-fit", &theme()).unwrap().declaration,
            "width:fit-content;height:fit-content;"
        );
    }

    #[test]
    fn resolves_filter_and_perspective_keywords() {
        let theme = theme();
        assert_eq!(
            resolve_utility("blur-none", &theme).unwrap().declaration,
            "--tw-blur: ;filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
        );
        assert_eq!(
            resolve_utility("perspective-dramatic", &theme)
                .unwrap()
                .declaration,
            "perspective:100px;"
        );
        assert_eq!(
            resolve_utility("perspective-origin-top-left", &theme)
                .unwrap()
                .declaration,
            "perspective-origin:top left;"
        );
    }

    #[test]
    fn lets_user_css_override_compatibility_fallback_utility_names() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@utility size-* { inline-size: --value(--spacing-*); block-size: --value(--spacing-*); }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_utility("size-4", &theme).unwrap().declaration,
            "inline-size: calc(var(--spacing) * 4); block-size: calc(var(--spacing) * 4);"
        );
    }

    #[test]
    fn resolves_animation_size_and_grid_columns_from_css_value_domains() {
        let theme = theme();

        assert_eq!(
            resolve_utility("animate-spin", &theme).unwrap().declaration,
            "animation:var(--animate-spin);"
        );
        assert_eq!(
            resolve_utility("size-[12px]", &theme).unwrap().declaration,
            "width:12px;height:12px;"
        );
        assert_eq!(
            resolve_utility("grid-cols-13", &theme).unwrap().declaration,
            "grid-template-columns:repeat(13,minmax(0,1fr));"
        );
    }

    #[test]
    fn resolves_translate_and_scale_fallbacks_from_css_value_domains() {
        let theme = theme();

        assert_eq!(
            resolve_utility("translate-x-[12px]", &theme)
                .unwrap()
                .declaration,
            "--tw-translate-x:12px;translate:var(--tw-translate-x) var(--tw-translate-y);"
        );
        assert_eq!(
            resolve_utility("-translate-y-[12px]", &theme)
                .unwrap()
                .declaration,
            "--tw-translate-y:calc(12px * -1);translate:var(--tw-translate-x) var(--tw-translate-y);"
        );
        assert_eq!(
            resolve_utility("scale-[0.8]", &theme).unwrap().declaration,
            "--tw-scale-x:0.8;--tw-scale-y:0.8;--tw-scale-z:0.8;scale:var(--tw-scale-x) var(--tw-scale-y);"
        );
    }

    #[test]
    fn resolves_user_block_variants_with_the_same_slot_syntax_as_built_ins() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant pressed { &:active { @slot; } }\n@variant narrow { @media (width < 40rem) { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(
                &parse_candidate("pressed:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate("&:active".to_owned()))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("narrow:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width < 40rem)".to_owned()))
        );
    }

    #[test]
    fn resolves_wildcard_selector_variants_from_application_css() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@variant state-* { &[data-state=--value()] { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(
                &parse_candidate("state-open:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[data-state=open]".to_owned()
            ))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("aria-expanded:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::SelectorTemplate(
                "&[aria-expanded=\"true\"]".to_owned()
            ))
        );
    }

    #[test]
    fn resolves_wildcard_conditional_variants_from_css() {
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --breakpoint-narrow: 40rem; }\n@variant narrow-* { @media (width < --value(--breakpoint-*)) { @slot; } }",
        );
        let theme = parse_tailwind_theme(&source).unwrap();

        assert_eq!(
            resolve_variant(&parse_candidate("min-md:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Media("(width >= 48rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
            Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
        );
        assert_eq!(
            resolve_variant(
                &parse_candidate("narrow-narrow:flex").unwrap().variants[0],
                &theme
            ),
            Some(VariantApplication::Media("(width < 40rem)".to_owned()))
        );
    }
}
