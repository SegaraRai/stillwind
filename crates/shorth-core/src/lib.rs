//! Public orchestration API for the first compiler slice.

use shorth_grammar::lex_utilities;
use shorth_ir::StyleInput;
use shorth_source::{Diagnostic, SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompileResult {
    pub css: String,
    pub classes: Vec<GeneratedClass>,
    pub diagnostics: Vec<Diagnostic>,
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

        for call in parsed.module.style_calls {
            let mut declarations = Vec::new();
            let mut canonical_input = String::new();
            for input in call.inputs {
                let StyleInput::UtilityString(input) = input;
                for utility in lex_utilities(&input) {
                    canonical_input.push_str(&utility.value.raw);
                    canonical_input.push('\0');
                    match resolve_utility(&utility.value.raw, &theme) {
                        Some(declaration) => declarations.push(declaration),
                        None => result.diagnostics.push(Diagnostic::error(
                            "SH001",
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        )),
                    }
                }
            }

            if declarations.is_empty() {
                continue;
            }
            let class_name = class_name(&canonical_input);
            result.css.push('.');
            result.css.push_str(&class_name);
            result.css.push('{');
            result.css.push_str(&declarations.concat());
            result.css.push_str("}\n");
            result.classes.push(GeneratedClass {
                class_name,
                call_span: call.span,
            });
        }

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == shorth_source::Severity::Error)
        {
            result.css.clear();
            result.classes.clear();
        }

        result
    }
}

fn resolve_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<String> {
    match utility {
        "inline-flex" => Some("display:inline-flex;".to_owned()),
        "items-center" => Some("align-items:center;".to_owned()),
        "text-center" => Some("text-align:center;".to_owned()),
        _ => resolve_themed_utility(utility, theme),
    }
}

fn resolve_themed_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<String> {
    for (prefix, namespace, property) in [
        ("bg-", "color", "background-color"),
        ("text-", "color", "color"),
        ("px-", "spacing", "padding-inline"),
        ("rounded-", "radius", "border-radius"),
    ] {
        let Some(token) = utility.strip_prefix(prefix) else {
            continue;
        };
        if theme
            .tokens
            .iter()
            .any(|entry| entry.name.value == format!("--{namespace}-{token}"))
        {
            return Some(format!("{property}:var(--{namespace}-{token});"));
        }
    }
    None
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
