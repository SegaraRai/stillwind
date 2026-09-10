use std::collections::BTreeSet;

use stillwind_css::{CssOutputNode, CssOutputTemplate, ThemeIndex, ThemeNamespace, ThemeTokenKind};
use stillwind_tailwind::builtin_omit_token_prefixes;

use crate::CompilerOptions;

use super::{
    authored_directives::expand_authored_directives, keyframes::extract_authored_keyframes,
    properties::render_used_properties, render::StylesheetError,
};

pub(super) fn render(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, StylesheetError> {
    let authored = if document {
        render_authored(theme.custom_css(), theme)?
    } else {
        String::new()
    };
    let used = format!("{utility_css}{authored}");
    let required =
        theme.tokens_required_by_with_omit_rules(&used, false, builtin_omit_token_prefixes(theme));
    let tokens = theme
        .tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            required
                .iter()
                .any(|required| std::ptr::eq(*required, token))
                .then_some(index)
        })
        .collect();
    let animations = required
        .iter()
        .filter_map(|token| match token.kind {
            ThemeTokenKind::Token {
                namespace: ThemeNamespace::Animate,
                ..
            } => token
                .value
                .value
                .split_whitespace()
                .next()
                .map(str::to_owned),
            _ => None,
        })
        .collect();
    let mut context = Context {
        theme,
        utility_css,
        options,
        document,
        tokens,
        animations,
        slots: 0,
    };
    let mut css = context.nodes(&template.nodes)?;
    if !utility_css.is_empty() && context.slots == 0 {
        return Err(StylesheetError::MissingUtilitiesSlot);
    }
    let properties = render_used_properties(theme, &css, options)?;
    if !properties.is_empty() {
        css.insert_str(0, "@layer properties;\n");
        css.push_str(&properties);
    }
    Ok(css)
}

struct Context<'a> {
    theme: &'a ThemeIndex,
    utility_css: &'a str,
    options: &'a CompilerOptions,
    document: bool,
    tokens: BTreeSet<usize>,
    animations: Vec<String>,
    slots: usize,
}

fn render_authored(css: &str, theme: &ThemeIndex) -> Result<String, StylesheetError> {
    theme
        .resolve_theme_references(&expand_authored_directives(css, theme))
        .ok_or(StylesheetError::UnresolvedThemeReference)
}

impl Context<'_> {
    fn nodes(&mut self, nodes: &[CssOutputNode]) -> Result<String, StylesheetError> {
        let mut output = String::new();
        for node in nodes {
            match node {
                CssOutputNode::Authored(css) if self.document => {
                    output.push_str(&render_authored(css, self.theme)?);
                }
                CssOutputNode::Authored(_) => {}
                CssOutputNode::Utilities => {
                    self.slots += 1;
                    output.push_str(self.utility_css);
                }
                CssOutputNode::Group { header, children } => {
                    let body = self.nodes(children)?;
                    if self.document || !body.trim().is_empty() {
                        output.push_str(header);
                        output.push('{');
                        output.push_str(&body);
                        output.push('}');
                    }
                }
                CssOutputNode::Theme { tokens, keyframes } => {
                    let mut declarations = String::new();
                    for index in tokens.iter().filter(|index| self.tokens.contains(index)) {
                        if let Some(token) = self.theme.tokens.get(*index)
                            && let Some(value) =
                                self.theme.resolve_theme_references(&token.value.value)
                        {
                            declarations.push_str(&token.name.value);
                            declarations.push(':');
                            declarations.push_str(&value);
                            declarations.push(';');
                        }
                    }
                    if !declarations.is_empty() {
                        output.push_str(":root,:host{");
                        output.push_str(&declarations);
                        output.push('}');
                    }
                    output.push_str(&self.options.rewrite_generated(&extract_authored_keyframes(
                        keyframes,
                        &self.animations,
                    )));
                }
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillwind_css::parse_theme;
    use stillwind_source::{FileId, SourceFile};

    #[test]
    fn slots_and_support_preserve_authored_conditions_in_document_and_module_outputs() {
        let source = SourceFile::new(FileId(1), "entry.css", "@layer theme, utilities; @layer theme { @theme { --color-brand: red; --unused:blue; } } .before{display:block;} @layer utilities { @supports (display:grid) { @media print { @stillwind-slot utilities; } } } .after{color:blue;}").unwrap();
        let theme = parse_theme(&source).unwrap();
        let template = &theme.output_template;
        let utility = ".x{color:var(--color-brand);}";
        let css = render(template, &theme, utility, &CompilerOptions::default(), true).unwrap();
        assert!(css.contains("@layer theme{:root,:host{--color-brand:red;}}"));
        assert!(css.contains("@layer utilities{@supports (display:grid){@media print{.x{color:var(--color-brand);}}}}"));
        assert!(css.find(".before").unwrap() < css.find(".x{").unwrap());
        assert!(css.find(".x{").unwrap() < css.find(".after").unwrap());
        assert!(!css.contains("--unused"));
        let module = render(
            template,
            &theme,
            utility,
            &CompilerOptions::default(),
            false,
        )
        .unwrap();
        assert!(!module.contains(".before"));
        assert!(!module.contains(".after"));
        assert!(module.contains("@media print{.x{"));
        assert!(module.contains("--color-brand:red;"));
    }

    #[test]
    fn missing_slots_are_reported_and_no_import_has_no_reset_or_defaults() {
        let source = SourceFile::new(FileId(1), "entry.css", ".a{color:red;}").unwrap();
        let theme = parse_theme(&source).unwrap();
        let template = &theme.output_template;
        assert_eq!(
            render(template, &theme, "", &CompilerOptions::default(), true).unwrap(),
            ".a{color:red;}"
        );
        assert!(matches!(
            render(
                template,
                &theme,
                ".x{display:block}",
                &CompilerOptions::default(),
                true
            ),
            Err(StylesheetError::MissingUtilitiesSlot)
        ));
        assert!(theme.registry.utilities.is_empty());
        assert!(theme.tokens.is_empty());
    }
}
