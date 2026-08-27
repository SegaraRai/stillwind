//! Public orchestration API for the first compiler slice.

use std::fmt::Write;

use serde::Serialize;
use shorth_grammar::lex_utilities;
use shorth_ir::{AttributeEntry, StyleInput};
use shorth_source::{Diagnostic, SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    pub classes: Vec<GeneratedClass>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, String> {
        let mut transforms = self.transforms.clone();
        transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

        let mut output = source.text().to_owned();
        let mut previous_start = u32::MAX;
        for transform in transforms {
            if transform.call_span.file != source.id()
                || transform.call_span.end.0 > previous_start
                || transform.call_span.end.0 as usize > output.len()
            {
                return Err("invalid or overlapping shorth transform plan".to_owned());
            }
            let start = transform.call_span.start.0 as usize;
            let end = transform.call_span.end.0 as usize;
            output.replace_range(start..end, &transform.replacement);
            previous_start = transform.call_span.start.0;
        }
        Ok(output)
    }
}

pub struct Compiler;

impl Compiler {
    pub fn check_javascript(source: &SourceFile) -> Vec<Diagnostic> {
        shorth_js_oxc::parse(source).diagnostics
    }

    pub fn analyze_javascript(source: &SourceFile) -> shorth_js_oxc::ParsedJavaScript {
        shorth_js_oxc::parse(source)
    }

    /// Reads the project's CSS-first theme surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the stylesheet is invalid or contains an
    /// unterminated `@theme` block.
    pub fn read_theme(
        source: &SourceFile,
    ) -> Result<shorth_css::ThemeIndex, shorth_css::ThemeError> {
        shorth_css::parse_theme(source)
    }

    /// Compiles the first static React subset into CSS.
    ///
    /// This temporary API deliberately supports only literal utility-string
    /// arguments. Conditions, attributes, selectors, and transforms are added
    /// in successive compiler slices.
    #[must_use]
    pub fn compile_react(source: &SourceFile, theme_source: &SourceFile) -> CompileResult {
        let parsed = Self::analyze_javascript(source);
        let mut result = CompileResult {
            diagnostics: parsed.diagnostics,
            ..CompileResult::default()
        };
        let theme = match Self::read_theme(theme_source) {
            Ok(theme) => theme,
            Err(error) => {
                result.diagnostics.push(Diagnostic::error(
                    "SHCSS001",
                    error.to_string(),
                    theme_source.full_span(),
                ));
                return result;
            }
        };

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == shorth_source::Severity::Error)
        {
            return result;
        }

        let mut utility_css = String::new();
        for call in parsed.module.style_calls {
            let mut branches = vec![(None, String::new(), Vec::new())];
            let mut attributes = Vec::new();
            for input in call.inputs {
                let (guard, selector, input) = match input {
                    StyleInput::UtilityString(input) => (None, None, input),
                    StyleInput::ConditionalUtilityString { guard, value } => {
                        (Some(guard), None, value)
                    }
                    StyleInput::Attributes(entries) => {
                        attributes.extend(entries);
                        continue;
                    }
                    StyleInput::SelectorUtilityString { selector, value } => {
                        (None, Some(selector.value), value)
                    }
                    StyleInput::ConditionalSelectorUtilityString {
                        selector,
                        guard,
                        value,
                    } => (Some(guard), Some(selector.value), value),
                };
                let branch_index = if let Some(guard) = guard {
                    branches.push((Some(guard), String::new(), Vec::new()));
                    branches.len() - 1
                } else {
                    0
                };
                let branch = &mut branches[branch_index];
                for utility in lex_utilities(&input) {
                    if let Some(selector) = &selector {
                        branch.1.push_str(selector);
                        branch.1.push('\0');
                    }
                    branch.1.push_str(&utility.value.raw);
                    branch.1.push('\0');
                    match resolve_utility(&utility.value.raw, &theme) {
                        Some(mut utility) => {
                            utility.selector = selector.clone();
                            branch.2.push(utility);
                        }
                        None => result.diagnostics.push(Diagnostic::error(
                            "SH001",
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        )),
                    }
                }
            }

            let mut call_classes = Vec::new();
            for (guard, canonical_input, utilities) in branches {
                if utilities.is_empty() {
                    continue;
                }
                let class_name = class_name(&canonical_input);
                emit_utility_rules(&mut utility_css, &class_name, &utilities, &theme);
                let generated_class = GeneratedClass {
                    class_name,
                    call_span: call.span,
                    guard,
                };
                call_classes.push(generated_class.clone());
                result.classes.push(generated_class);
            }
            if let Some(replacement) = render_react_props(source, &call_classes, &attributes) {
                result.transforms.push(PropsTransform {
                    call_span: call.span,
                    replacement,
                });
            }
        }

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == shorth_source::Severity::Error)
        {
            result.css.clear();
            result.classes.clear();
            result.transforms.clear();
        } else if !result.classes.is_empty() {
            result.css = render_stylesheet(&theme, &utility_css);
        }

        result
    }
}

fn render_react_props(
    source: &SourceFile,
    classes: &[GeneratedClass],
    attributes: &[AttributeEntry],
) -> Option<String> {
    let mut entries = Vec::new();
    if !classes.is_empty() {
        let class_names = classes
            .iter()
            .map(|class| match class.guard {
                Some(guard) => source
                    .slice(guard)
                    .ok()
                    .map(|guard| format!("{guard}&&\"{}\"", class.class_name)),
                None => Some(format!("\"{}\"", class.class_name)),
            })
            .collect::<Option<Vec<_>>>()?;
        let class_value = if class_names.len() == 1 {
            class_names.into_iter().next()?
        } else {
            format!("stillwindClsx({})", class_names.join(","))
        };
        entries.push(format!("className:{class_value}"));
    }
    for attribute in attributes {
        let value = source.slice(attribute.value).ok()?;
        entries.push(format!("\"{}\":{value}", attribute.name.value));
    }
    Some(format!("{{{}}}", entries.join(",")))
}

#[derive(Clone, Debug)]
struct CompiledUtility {
    variants: Vec<String>,
    declaration: String,
    selector: Option<String>,
}

fn resolve_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<CompiledUtility> {
    let (variants, base) = split_variants(utility);
    let declaration = match base {
        "block" => Some("display:block;".to_owned()),
        "inline-block" => Some("display:inline-block;".to_owned()),
        "flex" => Some("display:flex;".to_owned()),
        "inline-flex" => Some("display:inline-flex;".to_owned()),
        "grid" => Some("display:grid;".to_owned()),
        "hidden" => Some("display:none;".to_owned()),
        "items-center" => Some("align-items:center;".to_owned()),
        "items-start" => Some("align-items:flex-start;".to_owned()),
        "items-end" => Some("align-items:flex-end;".to_owned()),
        "justify-center" => Some("justify-content:center;".to_owned()),
        "justify-between" => Some("justify-content:space-between;".to_owned()),
        "text-center" => Some("text-align:center;".to_owned()),
        "text-left" => Some("text-align:left;".to_owned()),
        "font-medium" => Some("font-weight:500;".to_owned()),
        "font-semibold" => Some("font-weight:600;".to_owned()),
        "font-bold" => Some("font-weight:700;".to_owned()),
        "cursor-wait" => Some("cursor:wait;".to_owned()),
        "cursor-pointer" => Some("cursor:pointer;".to_owned()),
        "relative" => Some("position:relative;".to_owned()),
        "absolute" => Some("position:absolute;".to_owned()),
        "overflow-hidden" => Some("overflow:hidden;".to_owned()),
        "transition" => Some("transition-property:color,background-color,border-color,outline-color,text-decoration-color,fill,stroke,opacity,box-shadow,transform,filter,backdrop-filter;transition-duration:.15s;transition-timing-function:cubic-bezier(.4,0,.2,1);".to_owned()),
        "opacity-60" => Some("opacity:.6;".to_owned()),
        "opacity-0" => Some("opacity:0;".to_owned()),
        "opacity-100" => Some("opacity:1;".to_owned()),
        _ => resolve_themed_utility(base, theme),
    }?;
    Some(CompiledUtility {
        variants,
        declaration,
        selector: None,
    })
}

fn resolve_themed_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<String> {
    if let Some((color, opacity)) = utility
        .strip_prefix("bg-")
        .and_then(|value| value.rsplit_once('/'))
        && has_theme_token(theme, "color", color)
    {
        return Some(format!(
            "background-color:color-mix(in oklab,var(--color-{color}) {opacity}%,transparent);"
        ));
    }
    for (prefix, namespace, property) in [
        ("bg-", "color", "background-color"),
        ("text-", "color", "color"),
        ("border-", "color", "border-color"),
        ("p-", "spacing", "padding"),
        ("px-", "spacing", "padding-inline"),
        ("py-", "spacing", "padding-block"),
        ("pt-", "spacing", "padding-top"),
        ("pr-", "spacing", "padding-right"),
        ("pb-", "spacing", "padding-bottom"),
        ("pl-", "spacing", "padding-left"),
        ("m-", "spacing", "margin"),
        ("mx-", "spacing", "margin-inline"),
        ("my-", "spacing", "margin-block"),
        ("gap-", "spacing", "gap"),
        ("w-", "spacing", "width"),
        ("h-", "spacing", "height"),
        ("rounded-", "radius", "border-radius"),
        ("shadow-", "shadow", "box-shadow"),
        ("text-", "text", "font-size"),
    ] {
        let Some(token) = utility.strip_prefix(prefix) else {
            continue;
        };
        if has_theme_token(theme, namespace, token) {
            return Some(format!("{property}:var(--{namespace}-{token});"));
        }
    }
    None
}

fn has_theme_token(theme: &shorth_css::ThemeIndex, namespace: &str, token: &str) -> bool {
    theme
        .tokens
        .iter()
        .any(|entry| entry.name.value == format!("--{namespace}-{token}"))
}

fn split_variants(utility: &str) -> (Vec<String>, &str) {
    let mut depth = 0_u8;
    let mut start = 0;
    let mut variants = Vec::new();
    for (index, character) in utility.char_indices() {
        match character {
            '[' => depth = depth.saturating_add(1),
            ']' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => {
                variants.push(utility[start..index].to_owned());
                start = index + 1;
            }
            _ => {}
        }
    }
    (variants, &utility[start..])
}

fn emit_utility_rules(
    output: &mut String,
    class_name: &str,
    utilities: &[CompiledUtility],
    theme: &shorth_css::ThemeIndex,
) {
    let base = utilities
        .iter()
        .filter(|utility| utility.variants.is_empty() && utility.selector.is_none())
        .map(|utility| utility.declaration.as_str())
        .collect::<String>();
    if !base.is_empty() {
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        output.push_str(&base);
        output.push('}');
    }
    for utility in utilities
        .iter()
        .filter(|utility| !utility.variants.is_empty() || utility.selector.is_some())
    {
        let mut selector = utility
            .selector
            .as_deref()
            .map(|selector| selector.replace('&', &format!(".{class_name}")))
            .unwrap_or_else(|| format!(".{class_name}"));
        let mut media = Vec::new();
        for variant in &utility.variants {
            match variant.as_str() {
                "hover" => selector.push_str(":hover"),
                "focus" => selector.push_str(":focus"),
                "focus-visible" => selector.push_str(":focus-visible"),
                "active" => selector.push_str(":active"),
                "disabled" => selector.push_str(":disabled"),
                "dark" => media.push("(prefers-color-scheme: dark)".to_owned()),
                value if value.starts_with("aria-[") && value.ends_with(']') => {
                    if let Some((name, expected)) = value[6..value.len() - 1].split_once('=') {
                        write!(&mut selector, "[aria-{name}=\"{expected}\"]")
                            .expect("writing to a String cannot fail");
                    }
                }
                value if value.starts_with("data-[") && value.ends_with(']') => {
                    if let Some((name, expected)) = value[6..value.len() - 1].split_once('=') {
                        write!(&mut selector, "[data-{name}=\"{expected}\"]")
                            .expect("writing to a String cannot fail");
                    }
                }
                value => {
                    if let Some(token) = theme
                        .tokens
                        .iter()
                        .find(|token| token.name.value == format!("--breakpoint-{value}"))
                    {
                        media.push(format!("(width >= {})", token.value.value));
                    }
                }
            }
        }
        let mut rule = format!("{selector}{{{}}}", utility.declaration);
        for condition in media.into_iter().rev() {
            rule = format!("@media {condition}{{{rule}}}");
        }
        output.push_str(&rule);
    }
}

fn render_stylesheet(theme: &shorth_css::ThemeIndex, utility_css: &str) -> String {
    let mut css = String::from("@layer theme, base, components, utilities;\n@layer theme{:root{");
    for token in &theme.tokens {
        css.push_str(&token.name.value);
        css.push(':');
        css.push_str(&token.value.value);
        css.push(';');
    }
    css.push_str("}}\n@layer utilities{");
    css.push_str(utility_css);
    css.push_str("}\n");
    css
}

fn class_name(canonical_input: &str) -> String {
    let hash = blake3::hash(canonical_input.as_bytes());
    format!("_s_{}", &hash.to_hex()[..8])
}

#[cfg(test)]
mod tests {
    use shorth_source::FileId;

    use super::*;

    #[test]
    fn exposes_the_first_frontends_through_one_compiler_api() {
        let js = SourceFile::new(FileId(1), "App.tsx", "export const App = () => <main />;");
        let css = SourceFile::new(FileId(2), "shorth.css", "@theme { --color-accent: red; }");

        assert!(Compiler::check_javascript(&js).is_empty());
        assert_eq!(Compiler::read_theme(&css).unwrap().tokens.len(), 1);
    }
}
