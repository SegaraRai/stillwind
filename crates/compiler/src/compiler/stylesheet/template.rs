//! Assembly of the final stylesheet from the design system's output template.
//!
//! [`render_mapped`] combines a design-system template with one producer's rules,
//! generated keyframes, and registration dependencies. It selects the required theme
//! declarations and registrations, then inserts them at the template's slots. The
//! template supplies selectors, layers, and conditions; this module does not choose
//! browser targets or minify values.
//!
//! Document output includes authored rules. Carrier output includes generated support
//! and layer reservations, but omits independent authored rules. Both use the same
//! anonymous-layer identities. [`render`] is the raw-CSS entry point without input
//! metadata; generated artifacts use [`render_mapped`] to preserve their placement,
//! dependencies, and source mappings.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::sync::Arc;

use cssparser::{Parser, Token};

use stillwind_css::{
    CssOutputNode, CssOutputTemplate, custom_property_usage, take_property_requirements,
};
use stillwind_design_system::{
    CssSourceGraph, ThemeIndex, ThemeNamespace, ThemeTokenKind, builtin_omit_token_prefixes,
    builtin_property_definitions,
};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{SourceFile, Span};

use super::authored_directives::expand_authored_directives;
use super::keyframes::extract_authored_keyframes;
use super::properties::{PropertyOutput, render_used_properties};
use crate::{CompilerOptions, MappedStylesheet};

/// Assembles raw CSS against `template` and returns the document or carrier text.
///
/// `theme` supplies the definitions indexed by the template. `utility_css` has no
/// generated-keyframe classification or explicit registration metadata; callers with
/// generated artifacts must use [`render_mapped`]. `document` includes authored rules
/// when true. Neither the template nor the input CSS is modified.
///
/// # Errors
///
/// Returns the authored-CSS, registration, and missing-slot errors from [`render_mapped`].
pub(super) fn render(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, CompileErrorKind> {
    render_mapped(
        template,
        theme,
        &MappedStylesheet {
            css: utility_css.into(),
            ..MappedStylesheet::default()
        },
        options,
        document,
        None,
    )
    .map(|output| output.css)
}

/// Renders a stylesheet with source provenance, for a document or a single carrier.
///
/// `template` must index the supplied `theme`; `utility_css` carries rule text,
/// separately classified keyframes, and explicit registration dependencies. The
/// returned CSS follows template order, including repeated slot occurrences.
/// `document` controls whether independent authored rules participate.
///
/// `sources` resolves template origins to source buffers. Without it, template text
/// is unmapped; mappings already attached to input rules and keyframes are retained.
///
/// # Errors
///
/// Returns [`CompileErrorKind`] for malformed or unknown registration requirements,
/// unresolved authored theme references, invalid property-usage syntax, or a missing
/// destination for a nonempty generated contribution.
#[allow(clippy::too_many_lines)]
pub(crate) fn render_mapped(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &MappedStylesheet,
    options: &CompilerOptions,
    document: bool,
    sources: Option<&CssSourceGraph>,
) -> Result<MappedStylesheet, CompileErrorKind> {
    let mut explicit = utility_css.required_properties.clone();
    let authored = if document {
        render_authored(theme.custom_css(), theme, &mut explicit)?
    } else {
        String::new()
    };
    let mut used = format!("{}{authored}", utility_css.css);
    for frames in &utility_css.keyframes {
        used.push_str(&frames.css);
    }
    let builtin = builtin_property_definitions(theme);
    if explicit.iter().any(|name| {
        !builtin.iter().chain(&theme.properties).any(|property| {
            property.name == *name || options.rewrite_generated(&property.name) == *name
        })
    }) {
        return Err(CompileErrorKind::InvalidPropertyRequirement);
    }
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
    let animations: Vec<String> = required
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
    let mut definitions = Vec::new();
    collect_theme_nodes(&template.nodes, &mut definitions);
    // Include selected theme values and frames in registration demand even when
    // their slots are conditional: placement does not remove their dependencies.
    let mut declarations = String::new();
    for token in &required {
        let _ = write!(
            declarations,
            "{}:{};",
            token.name.value,
            theme
                .resolve_theme_references(&token.value.value)
                .unwrap_or_else(|| token.value.value.clone())
        );
    }
    for definition in &definitions {
        if let CssOutputNode::Theme { keyframes, .. } = definition {
            used.push_str(&extract_authored_keyframes(keyframes, &animations));
        }
    }
    let mut property_usage =
        custom_property_usage(&used).map_err(|error| CompileErrorKind::PropertyUsage {
            detail: error.0.to_string(),
        })?;
    property_usage
        .add_declarations(&declarations)
        .map_err(|error| CompileErrorKind::PropertyUsage {
            detail: error.0.to_string(),
        })?;
    let properties = render_used_properties(theme, &property_usage, options, &explicit);
    if !properties.registrations.is_empty()
        && !has_slot(&template.nodes, &CssOutputNode::Properties)
    {
        return Err(CompileErrorKind::MissingPropertySlot {
            slot: "properties".into(),
        });
    }
    if !properties.initials.is_empty()
        && !has_slot(&template.nodes, &CssOutputNode::PropertyInitials)
    {
        return Err(CompileErrorKind::MissingPropertySlot {
            slot: "property-initials".into(),
        });
    }
    let mut context = Context {
        theme,
        utility_css,
        options,
        document,
        tokens,
        animations,
        slots: 0,
        theme_variables: MappedStylesheet::default(),
        keyframes: MappedStylesheet::default(),
        properties,
        layer_prefix: anonymous_layer_prefix(template, options),
        anonymous_layers: 0,
        sources,
    };
    for definition in definitions {
        if let CssOutputNode::Theme {
            tokens,
            keyframes,
            origin,
        } = definition
        {
            let variables = context.theme_fragment(tokens, keyframes, *origin, true, false);
            let frames = context.theme_fragment(tokens, keyframes, *origin, false, true);
            context.theme_variables.append(&variables);
            context.keyframes.append(&frames);
        }
    }
    for frames in &utility_css.keyframes {
        context.keyframes.append(frames);
    }
    for (needed, slot, name) in [
        (
            !context.theme_variables.css.is_empty(),
            CssOutputNode::ThemeVariables,
            "theme",
        ),
        (
            !context.keyframes.css.is_empty(),
            CssOutputNode::Keyframes,
            "keyframes",
        ),
    ] {
        if needed && !has_slot(&template.nodes, &slot) {
            return Err(CompileErrorKind::MissingPropertySlot { slot: name.into() });
        }
    }
    let css = context.nodes(&template.nodes)?;
    if !utility_css.css.is_empty() && context.slots == 0 {
        return Err(CompileErrorKind::MissingUtilitiesSlot);
    }
    Ok(css)
}

/// State carried through one template walk.
struct Context<'a> {
    /// Resolved theme containing token definitions and the output template.
    theme: &'a ThemeIndex,
    /// Mapped utility contribution inserted at utilities slots.
    utility_css: &'a MappedStylesheet,
    /// Generated-name configuration used during template expansion.
    options: &'a CompilerOptions,
    /// Whether authored document rules are included rather than only carrier support.
    document: bool,
    /// Selected indices into theme.tokens, traversed in ascending definition order.
    tokens: BTreeSet<usize>,
    /// Requested animation names used to select keyframe definitions.
    animations: Vec<String>,
    /// Number of utilities slots visited; zero rejects a nonempty utility contribution.
    slots: usize,
    /// Mapped declarations inserted into theme slots, without selector braces.
    theme_variables: MappedStylesheet,
    /// Mapped complete animation rules inserted into keyframes slots.
    keyframes: MappedStylesheet,
    /// Registration and fallback fragments selected for the current contribution.
    properties: PropertyOutput,
    /// Generated prefix used to identify anonymous layers consistently during this walk.
    layer_prefix: String,
    /// Counter used to allocate distinct anonymous-layer identities.
    anonymous_layers: usize,
    /// Optional retained CSS source graph for resolving authored origin spans.
    sources: Option<&'a CssSourceGraph>,
}

impl Context<'_> {
    /// Resolves a template origin into a source buffer and span, when both are known.
    ///
    /// A missing source graph or an unknown file id drops the mapping; the CSS is still
    /// emitted.
    fn origin(&self, span: Option<Span>) -> Option<(Arc<SourceFile>, Span)> {
        let span = span?;
        let source = self
            .sources?
            .sources
            .iter()
            .find(|source| source.source.id() == span.file)?;
        Some((source.source.clone(), span))
    }

    /// Selects one theme block's live declarations and requested animation definitions.
    ///
    /// `tokens` contains indices into `self.theme`, in definition order. The two flags
    /// select declaration and keyframe output independently. Declarations retain their
    /// token origins; selected frames use the block's `origin`. The result has no
    /// selector or layer wrapper.
    fn theme_fragment(
        &self,
        tokens: &[usize],
        keyframes: &str,
        origin: Option<Span>,
        emit_tokens: bool,
        emit_keyframes: bool,
    ) -> MappedStylesheet {
        let mut output = MappedStylesheet::default();
        let mut declarations = MappedStylesheet::default();
        for index in tokens.iter().filter(|index| self.tokens.contains(index)) {
            if let Some(token) = self.theme.tokens.get(*index)
                && let Some(value) = self.theme.resolve_theme_references(&token.value.value)
            {
                declarations.text(&token.name.value, self.origin(Some(token.name.span)));
                declarations.text(":", None);
                declarations.text(&value, self.origin(Some(token.value.span)));
                declarations.text(";", None);
            }
        }
        if emit_tokens && !declarations.css.is_empty() {
            output.append(&declarations);
        }
        if emit_keyframes {
            output.text(
                &self
                    .options
                    .rewrite_generated(&extract_authored_keyframes(keyframes, &self.animations)),
                self.origin(origin),
            );
        }
        output
    }

    /// Renders a template node list into mapped CSS.
    ///
    /// Anonymous layers are numbered by a counter that runs over the whole template,
    /// including groups a carrier omits, so the same layer gets the same name in every
    /// output that contains it. A carrier that omits an empty layer still emits the
    /// layer statement, because dropping it would change the layer order the document
    /// establishes. The walk advances the anonymous-layer counter and counts utility
    /// slot occurrences, including occurrences whose contribution is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if an included authored node has an invalid registration
    /// directive or an unresolved theme reference.
    fn nodes(&mut self, nodes: &[CssOutputNode]) -> Result<MappedStylesheet, CompileErrorKind> {
        self.nodes_in_context(nodes, false)
    }

    /// Renders nodes in rule or declaration context, advancing the template counters.
    ///
    /// `declarations` retains authored declarations surrounding a slot even in a
    /// carrier: omitting them could change the meaning of that generated style rule.
    /// Children of an at-rule group are rendered in rule context.
    ///
    /// # Errors
    ///
    /// Returns errors from authored directive expansion and theme-reference resolution.
    fn nodes_in_context(
        &mut self,
        nodes: &[CssOutputNode],
        declarations: bool,
    ) -> Result<MappedStylesheet, CompileErrorKind> {
        let mut output = MappedStylesheet::default();
        for node in nodes {
            match node {
                CssOutputNode::LayerOrder { css, origin } => output.text(css, self.origin(*origin)),
                CssOutputNode::Authored { css, origin } if self.document || declarations => {
                    let authored = render_authored(css, self.theme, &mut Vec::new())?;
                    output.text(&authored, self.origin(*origin));
                }
                CssOutputNode::Authored { .. } | CssOutputNode::Theme { .. } => {}
                CssOutputNode::ThemeVariables => output.append(&self.theme_variables),
                CssOutputNode::Keyframes => output.append(&self.keyframes),
                CssOutputNode::Properties => output.text(&self.properties.registrations, None),
                CssOutputNode::PropertyInitials => output.text(&self.properties.initials, None),
                CssOutputNode::Style {
                    header,
                    children,
                    origin,
                } => {
                    let body = self.nodes_in_context(children, true)?;
                    if !body.css.is_empty() {
                        output.text(header, self.origin(*origin));
                        output.text("{", None);
                        output.append(&body);
                        output.text("}", None);
                    }
                }
                CssOutputNode::Utilities => {
                    self.slots += 1;
                    output.append(self.utility_css);
                }
                CssOutputNode::Group {
                    header,
                    children,
                    origin,
                } => {
                    // Number before filtering the body so separate carriers assign the
                    // same name to the same anonymous layer, even when one omits its rules.
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
            }
        }
        Ok(output)
    }
}

/// Tests for `slot` anywhere in the node list, including style and group children.
fn has_slot(nodes: &[CssOutputNode], slot: &CssOutputNode) -> bool {
    nodes.iter().any(|node| {
        node == slot
            || matches!(node, CssOutputNode::Group { children, .. } | CssOutputNode::Style { children, .. } if has_slot(children, slot))
    })
}

/// Appends theme definitions in depth-first source order without changing the tree.
fn collect_theme_nodes<'a>(nodes: &'a [CssOutputNode], output: &mut Vec<&'a CssOutputNode>) {
    for node in nodes {
        match node {
            CssOutputNode::Theme { .. } => output.push(node),
            CssOutputNode::Group { children, .. } | CssOutputNode::Style { children, .. } => {
                collect_theme_nodes(children, output);
            }
            _ => {}
        }
    }
}

/// Generated prefix for anonymous layers, stable for one template and class namespace.
///
/// The prefix hashes the template source identity and class namespace. Equal inputs
/// produce equal prefixes across carriers; different namespaces separate applications
/// sharing a template. The 16 hexadecimal hash digits reduce accidental collisions
/// with authored names but do not reserve a collision-free CSS namespace.
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

/// True when the header is an `@layer` with no name, which the template uses for
/// anonymous layers.
fn is_anonymous_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
        && parser.is_exhausted()
}

/// True when the header starts with the `@layer` at-keyword.
fn is_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
}

/// Returns expanded authored CSS and appends its explicit registration dependencies.
///
/// `required_properties` retains its existing entries and may gain duplicate names.
/// Requirements written in the input precede those collected from `@apply` expansion.
///
/// # Errors
///
/// Returns an error for a malformed requirement directive or an unresolved theme
/// reference. Dependencies already appended remain in the accumulator on error.
fn render_authored(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<String>,
) -> Result<String, CompileErrorKind> {
    let (css, requirements) =
        take_property_requirements(css).ok_or(CompileErrorKind::InvalidPropertyRequirement)?;
    required_properties.extend(requirements);
    theme
        .resolve_theme_references(&expand_authored_directives(
            &css,
            theme,
            required_properties,
        ))
        .ok_or(CompileErrorKind::UnresolvedThemeReference)
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::parse_theme;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    #[test]
    fn declaration_slots_keep_css_owned_selectors_conditions_and_order() {
        let source = SourceFile::new(
            FileId(1),
            "entry.css",
            r#"
            @theme { --brand: red; }
            @stillwind-property --n { syntax: "<number>"; inherits: false; initial-value: 0; }
            @media print { @layer defaults {
                .theme-scope { color:blue; @stillwind-slot theme; color:green; }
                .reset::before { @stillwind-slot property-initials; }
            } }
            @stillwind-slot properties;
            @stillwind-slot utilities;
        "#,
        )
        .unwrap();
        let theme = parse_theme(&source).unwrap();
        let css = render(
            &theme.output_template,
            &theme,
            ".x{color:var(--brand);opacity:var(--n)}",
            &CompilerOptions::default(),
            false,
        )
        .unwrap();
        assert!(
            css.contains("@media print{@layer defaults{.theme-scope{"),
            "{css}"
        );
        assert!(css.contains("--brand:red;"), "{css}");
        assert!(css.find("color:blue").unwrap() < css.find("--brand:red").unwrap());
        assert!(css.find("--brand:red").unwrap() < css.find("color:green").unwrap());
        assert!(css.contains(".reset::before{"), "{css}");
        assert!(!css.contains(":root") && !css.contains(":host") && !css.contains("::backdrop"));
        assert!(!css.contains("@stillwind-slot"));
    }

    #[test]
    fn generated_keyframes_keep_mapping_while_raw_css_keeps_its_placement() {
        let source = SourceFile::new(FileId(1), "entry.css", "@layer animations { @stillwind-slot keyframes; } @layer utilities { @stillwind-slot utilities; }").unwrap();
        let theme = parse_theme(&source).unwrap();
        let original =
            Arc::new(SourceFile::new(FileId(9), "日本.tsx", "日本 animation source").unwrap());
        let mut frames = MappedStylesheet::default();
        frames.text(
            "@keyframes generated{to{opacity:0}}",
            Some((original.clone(), original.full_span())),
        );
        let utilities = MappedStylesheet {
            css: "@media print{@keyframes authored{to{opacity:1}}}.x{content:'@keyframes fake{}'}"
                .into(),
            keyframes: vec![frames],
            ..MappedStylesheet::default()
        };
        let output = render_mapped(
            &theme.output_template,
            &theme,
            &utilities,
            &CompilerOptions::default(),
            true,
            None,
        )
        .unwrap();
        assert!(
            output
                .css
                .contains("@layer animations{@keyframes generated{")
        );
        assert!(
            output
                .css
                .contains("@layer utilities{@media print{@keyframes authored{")
        );
        assert!(output.css.contains("content:'@keyframes fake{}'"));
        let mapping = &output.mappings[0];
        assert_eq!(
            &output.css[mapping.generated.clone()],
            "@keyframes generated{to{opacity:0}}"
        );
        assert_eq!(mapping.original, 0..original.text().len());
        assert!(Arc::ptr_eq(&mapping.source, &original));
    }

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
        let source = SourceFile::new(FileId(1), "entry.css", "@layer theme, utilities; @layer theme { :root,:host { @stillwind-slot theme; } @theme { --color-brand: red; --unused:blue; } } .before{display:block;} @layer utilities { @supports (display:grid) { @media print { @stillwind-slot utilities; } } } .after{color:blue;}").unwrap();
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
    fn slots_relocate_definitions_without_hoisting_conditions_or_layers() {
        let source = SourceFile::new(FileId(1), "entry.css", r#"
            @layer theme, utilities;
            @layer theme { :root,:host { @stillwind-slot theme; } }
            @layer utilities { @stillwind-slot utilities; }
            @media print { @layer motion { @stillwind-slot keyframes; } }
            @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
            @theme { --color-brand: red; --animate-spin: spin 1s;
                @keyframes spin { to { opacity: 0; } }
            }
            @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
        "#).unwrap();
        let theme = parse_theme(&source).unwrap();
        let css = render(
            &theme.output_template,
            &theme,
            ".x{color:var(--color-brand);animation:var(--animate-spin);width:var(--length)}",
            &CompilerOptions::default(),
            true,
        )
        .unwrap();
        assert!(
            css.contains("@layer theme{:root,:host{--color-brand:red;--animate-spin:spin 1s;}}"),
            "{css}"
        );
        assert!(
            css.contains("@media print{@layer motion{@keyframes spin"),
            "{css}"
        );
        assert_eq!(css.matches("@keyframes spin").count(), 1);
        assert!(css.find("@layer utilities{").unwrap() < css.find("@keyframes spin").unwrap());
        assert!(css.find("@property --length").unwrap() > css.find("@keyframes spin").unwrap());
    }

    #[test]
    fn theme_declarations_activate_registrations_in_document_and_carrier_outputs() {
        let source = SourceFile::new(FileId(1), "entry.css", r#"
            @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
            @theme { --spacing-brand: var(--length); --spacing-unused: var(--unused); }
            @media print { :root { @stillwind-slot theme; } }
            @stillwind-slot utilities;
            @stillwind-slot properties;
            * { @stillwind-slot property-initials; }
        "#).unwrap();
        let theme = parse_theme(&source).unwrap();
        for document in [true, false] {
            let css = render(
                &theme.output_template,
                &theme,
                ".x{width:var(--spacing-brand)}",
                &CompilerOptions::default(),
                document,
            )
            .unwrap();
            assert!(css.contains("@property --length"), "{css}");
            assert!(
                css.contains("@media print{:root{--spacing-brand:var(--length);}}"),
                "{css}"
            );
            assert!(!css.contains("--unused"), "{css}");
        }
    }

    #[test]
    fn registrations_require_a_destination_instead_of_implicit_tail_output() {
        let source = SourceFile::new(FileId(1), "entry.css", r#"
            @stillwind-property --length { syntax: "<length>"; inherits: false; initial-value: 0px; }
            @stillwind-slot utilities;
        "#).unwrap();
        let theme = parse_theme(&source).unwrap();
        assert!(matches!(
            render(
                &theme.output_template,
                &theme,
                ".x{width:var(--length)}",
                &CompilerOptions::default(),
                true
            ),
            Err(CompileErrorKind::MissingPropertySlot { slot }) if slot == "properties"
        ));
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
            Err(CompileErrorKind::MissingUtilitiesSlot)
        ));
        assert!(theme.registry.utilities.is_empty());
        assert!(theme.tokens.is_empty());
    }
}
