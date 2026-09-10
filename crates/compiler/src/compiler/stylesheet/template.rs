use std::collections::BTreeSet;
use std::sync::Arc;

use cssparser::{Parser, Token};

use stillwind_css::{CssOutputNode, CssOutputTemplate};
use stillwind_design_system::{
    CssSourceGraph, ThemeIndex, ThemeNamespace, ThemeTokenKind, builtin_omit_token_prefixes,
};
use stillwind_source::{SourceFile, Span};

use super::authored_directives::expand_authored_directives;
use super::keyframes::extract_authored_keyframes;
use super::properties::render_used_properties;
use super::render::StylesheetError;
use crate::{CompilerOptions, MappedStylesheet};

pub(super) fn render(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, StylesheetError> {
    render_mapped(
        template,
        theme,
        &MappedStylesheet {
            css: utility_css.into(),
            mappings: Vec::new(),
        },
        options,
        document,
        None,
    )
    .map(|output| output.css)
}

pub(crate) fn render_mapped(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &MappedStylesheet,
    options: &CompilerOptions,
    document: bool,
    sources: Option<&CssSourceGraph>,
) -> Result<MappedStylesheet, StylesheetError> {
    let authored = if document {
        render_authored(theme.custom_css(), theme)?
    } else {
        String::new()
    };
    let used = format!("{}{authored}", utility_css.css);
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
        layer_prefix: anonymous_layer_prefix(template, options),
        anonymous_layers: 0,
        sources,
    };
    let mut css = context.nodes(&template.nodes)?;
    if !utility_css.css.is_empty() && context.slots == 0 {
        return Err(StylesheetError::MissingUtilitiesSlot);
    }
    let properties = render_used_properties(theme, &css.css, options)?;
    if !properties.is_empty() {
        css.prepend_unmapped("@layer properties;\n");
        css.text(&properties, None);
    }
    Ok(css)
}

struct Context<'a> {
    theme: &'a ThemeIndex,
    utility_css: &'a MappedStylesheet,
    options: &'a CompilerOptions,
    document: bool,
    tokens: BTreeSet<usize>,
    animations: Vec<String>,
    slots: usize,
    layer_prefix: String,
    anonymous_layers: usize,
    sources: Option<&'a CssSourceGraph>,
}

impl Context<'_> {
    fn origin(&self, span: Option<Span>) -> Option<(Arc<SourceFile>, Span)> {
        let span = span?;
        let source = self
            .sources?
            .sources
            .iter()
            .find(|source| source.source.id() == span.file)?;
        Some((source.source.clone(), span))
    }

    fn nodes(&mut self, nodes: &[CssOutputNode]) -> Result<MappedStylesheet, StylesheetError> {
        let mut output = MappedStylesheet::default();
        for node in nodes {
            match node {
                CssOutputNode::LayerOrder { css, origin } => output.text(css, self.origin(*origin)),
                CssOutputNode::Authored { css, origin } if self.document => {
                    output.text(&render_authored(css, self.theme)?, self.origin(*origin));
                }
                CssOutputNode::Authored { .. } => {}
                CssOutputNode::Utilities => {
                    self.slots += 1;
                    output.append(self.utility_css);
                }
                CssOutputNode::Group {
                    header,
                    children,
                    origin,
                } => {
                    // Count the complete template, including groups omitted from this carrier.
                    let header = if is_anonymous_layer(header) {
                        let index = self.anonymous_layers;
                        self.anonymous_layers += 1;
                        format!("@layer {}-{index}", self.layer_prefix)
                    } else {
                        header.clone()
                    };
                    let body = self.nodes(children)?;
                    if !self.document && body.css.trim().is_empty() && is_layer(&header) {
                        output.text(&header, self.origin(*origin));
                        output.text(";", None);
                    } else if self.document || !body.css.trim().is_empty() {
                        output.text(&header, self.origin(*origin));
                        output.text("{", None);
                        output.append(&body);
                        output.text("}", None);
                    }
                }
                CssOutputNode::Theme {
                    tokens,
                    keyframes,
                    origin,
                } => {
                    let mut declarations = MappedStylesheet::default();
                    for index in tokens.iter().filter(|index| self.tokens.contains(index)) {
                        if let Some(token) = self.theme.tokens.get(*index)
                            && let Some(value) =
                                self.theme.resolve_theme_references(&token.value.value)
                        {
                            declarations
                                .text(&token.name.value, self.origin(Some(token.name.span)));
                            declarations.text(":", None);
                            declarations.text(&value, self.origin(Some(token.value.span)));
                            declarations.text(";", None);
                        }
                    }
                    if !declarations.css.is_empty() {
                        output.text(":root,:host{", None);
                        output.append(&declarations);
                        output.text("}", None);
                    }
                    output.text(
                        &self.options.rewrite_generated(&extract_authored_keyframes(
                            keyframes,
                            &self.animations,
                        )),
                        self.origin(*origin),
                    );
                }
            }
        }
        Ok(output)
    }
}

fn anonymous_layer_prefix(template: &CssOutputTemplate, options: &CompilerOptions) -> String {
    let identity = format!(
        "stillwind/layer/v1:{}:{}{}",
        options.class_namespace.len(),
        options.class_namespace,
        template.source_identity
    );
    format!(
        "sw-anon-{}",
        &blake3::hash(identity.as_bytes()).to_hex()[..16]
    )
}

fn is_anonymous_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
        && parser.is_exhausted()
}

fn is_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
}

fn render_authored(css: &str, theme: &ThemeIndex) -> Result<String, StylesheetError> {
    theme
        .resolve_theme_references(&expand_authored_directives(css, theme))
        .ok_or(StylesheetError::UnresolvedThemeReference)
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::parse_theme;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    #[test]
    fn anonymous_layers_share_identity_across_document_and_partial_carriers() {
        let source = SourceFile::new(FileId(1), "styles/entry.css", "@layer { .base{color:red} } @layer /* anonymous */ { @layer { @stillwind-slot utilities; } } @layer named { .end{color:blue} }").unwrap();
        let theme = parse_theme(&source).unwrap();
        let template = &theme.output_template;
        let options = CompilerOptions::default();
        let prefix = anonymous_layer_prefix(template, &options);
        let document = render(template, &theme, "", &options, true).unwrap();
        let module = render(template, &theme, ".x{color:green}", &options, false).unwrap();
        assert!(
            document.contains(&format!("@layer {prefix}-0{{")),
            "{document}"
        );
        assert!(document.contains(&format!("@layer {prefix}-1{{@layer {prefix}-2{{}}}}")));
        assert_eq!(
            module,
            format!(
                "@layer {prefix}-0;@layer {prefix}-1{{@layer {prefix}-2{{.x{{color:green}}}}}}@layer named;"
            )
        );
        assert!(document.contains("@layer named"));
        let isolated = CompilerOptions {
            class_namespace: "other-app".into(),
            ..options
        };
        assert_ne!(prefix, anonymous_layer_prefix(template, &isolated));
        let other_source = SourceFile::new(FileId(2), "other/entry.css", source.text()).unwrap();
        let other_theme = parse_theme(&other_source).unwrap();
        assert_ne!(
            prefix,
            anonymous_layer_prefix(&other_theme.output_template, &CompilerOptions::default())
        );
    }

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
