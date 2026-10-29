//! Selects and renders generated support independently of template placement.
//!
//! [`SupportSelection::select`] discovers token and registration demand before
//! lowering theme fragments. [`SupportSelection::render`] produces mapped slot
//! contributions after the caller has checked registration destinations. Selection
//! includes dependencies of theme values and requested theme keyframes regardless
//! of slot conditions. This module neither validates slot availability nor chooses
//! the selectors, layers, or conditions around its fragments.

use std::collections::BTreeSet;
use std::fmt::Write;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{CssOutputNode, CssOutputTemplate, ValueType, custom_property_usage};
use stillwind_design_system::{
    CssSourceGraph, ThemeIndex, ThemeValueRole, stillwind_omit_token_prefixes,
    stillwind_property_definitions,
};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::Span;

use super::super::keyframes::extract_authored_keyframes;
use super::super::properties::{PropertyOutput, render_used_properties};
use super::authored::render_authored;
use super::emit::resolve_origin;
use crate::compiler::conditionals::lower;
use crate::identifiers::IdentifierContext;
use crate::{CompilerOptions, MappedStylesheet};

/// Dependencies selected for one contribution, before theme fragment lowering.
///
/// Separating selection from rendering lets assembly report missing registration
/// destinations before conditional-lowering errors in theme fragments.
pub(super) struct SupportSelection<'a> {
    /// Theme indexed by the selected template definitions.
    theme: &'a ThemeIndex,
    /// Generated names and conditional-lowering strategy.
    options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    identifiers: &'a IdentifierContext,
    /// Selected indices in `theme.values()`, deduplicated and sorted numerically.
    /// Theme definitions determine the order in which these indices are emitted.
    tokens: BTreeSet<usize>,
    /// First whitespace-delimited words of selected animation-token values, in
    /// required-token order. Duplicate names are retained.
    animations: Vec<LeanStr>,
    /// Only [`CssOutputNode::Theme`] nodes, in depth-first source order across
    /// group and style children; no output wrappers are retained here.
    definitions: Vec<&'a CssOutputNode>,
    /// Selected registration and fallback output, before destination validation.
    pub(super) properties: PropertyOutput,
    /// Retained source buffers used for theme origins.
    sources: Option<&'a CssSourceGraph>,
}

impl<'a> SupportSelection<'a> {
    /// Discovers token and registration dependencies for one contribution.
    ///
    /// `template` must index `theme`. Utility rules and separate producer keyframes both
    /// contribute usage; `document` additionally includes expanded `theme.custom_css()`.
    /// Selected theme declarations and requested theme keyframes contribute registration
    /// usage even when their destination slots are conditional.
    ///
    /// `options` controls generated names and authored conditional lowering. `sources` is
    /// retained for later theme mappings. Inputs are unchanged; the returned selection
    /// contains registration fragments, but theme fragments have not been lowered yet.
    ///
    /// # Errors
    ///
    /// Returns malformed or unknown explicit registration requirements, unresolved
    /// authored theme references, invalid authored runtime conditionals, or invalid
    /// property-usage syntax. Slot availability is not checked.
    pub(super) fn select(
        template: &'a CssOutputTemplate,
        theme: &'a ThemeIndex,
        utility_css: &MappedStylesheet,
        options: &'a CompilerOptions,
        document: bool,
        sources: Option<&'a CssSourceGraph>,
        identifiers: &'a IdentifierContext,
    ) -> Result<Self, CompileDiagnostic> {
        let mut explicit = utility_css.required_properties.clone();
        let authored = if document {
            render_authored(
                theme.custom_css(),
                theme,
                &mut explicit,
                options,
                identifiers,
            )?
        } else {
            LeanString::new()
        };
        let mut used = format_args!("{}{authored}", utility_css.css).to_lean_string();
        for frames in &utility_css.keyframes {
            used.push_str(&frames.css);
        }
        let stillwind = stillwind_property_definitions(theme);
        if let Some(name) = explicit.iter().find(|name| {
            !stillwind.iter().chain(theme.properties()).any(|property| {
                property.name == **name || options.rewrite_generated(&property.name) == **name
            })
        }) {
            return Err(CompileDiagnostic::UnknownRequiredProperty { name: name.clone() });
        }
        let required = theme.values_required_by(&used, false, stillwind_omit_token_prefixes(theme));
        let tokens = theme
            .values()
            .iter()
            .enumerate()
            .filter_map(|(index, token)| {
                required
                    .iter()
                    .any(|required| std::ptr::eq(*required, token.as_ref()))
                    .then_some(index)
            })
            .collect();
        // A named token of a namespace that holds animations starts with the name of
        // the `@keyframes` rule it runs.
        let animations: Vec<LeanStr> = required
            .iter()
            .filter(|token| {
                token.role == ThemeValueRole::Token && matches!(
                    theme.namespaces().classify(&token.name.value),
                    Some((namespace, Some(_))) if namespace.types.contains(&ValueType::Animation)
                )
            })
            .filter_map(|token| {
                token
                    .value
                    .value
                    .split_whitespace()
                    .next()
                    .map(LeanStr::from)
            })
            .collect();
        let mut definitions = Vec::new();
        collect_theme_nodes(&template.nodes, &mut definitions);
        // Include selected theme values and frames in registration demand even when
        // their slots are conditional: placement does not remove their dependencies.
        let mut declarations = LeanString::new();
        for token in &required {
            if let Some(value) = resolve_theme_value(theme, &token.value.value)? {
                let _ = write!(declarations, "{}:{value};", token.name.value);
            }
        }
        for definition in &definitions {
            if let CssOutputNode::Theme { keyframes, .. } = definition {
                used.push_str(&extract_authored_keyframes(keyframes, &animations));
            }
        }
        let mut property_usage = custom_property_usage(&used).map_err(|error| {
            CompileDiagnostic::PropertyUsageParse {
                detail: error.0.to_lean_str(),
            }
        })?;
        property_usage
            .add_declarations(&declarations)
            .map_err(|error| CompileDiagnostic::PropertyUsageParse {
                detail: error.0.to_lean_str(),
            })?;
        let properties = render_used_properties(theme, &property_usage, options, &explicit);
        Ok(Self {
            theme,
            options,
            identifiers,
            tokens,
            animations,
            definitions,
            properties,
            sources,
        })
    }

    /// Consumes the selection to render theme support and append producer keyframes.
    ///
    /// `utility_css` must be the contribution used for selection. The caller checks
    /// registration destinations before invoking this method to preserve assembly error
    /// precedence. The method itself does not validate destinations.
    ///
    /// Theme fragments are rendered using the retained options and source graph. Producer
    /// keyframes are appended as supplied, preserving their order and mappings without
    /// conditional lowering. Inputs remain unchanged; the selection is consumed on both
    /// success and failure.
    ///
    /// # Errors
    ///
    /// Returns conditional-lowering errors for selected theme declarations or theme
    /// keyframes. No partial output is returned.
    pub(super) fn render(
        self,
        utility_css: &MappedStylesheet,
    ) -> Result<SupportOutput, CompileDiagnostic> {
        let mut theme_variables = MappedStylesheet::default();
        let mut keyframes = MappedStylesheet::default();
        for definition in &self.definitions {
            if let CssOutputNode::Theme {
                tokens,
                keyframes: frames,
                origin,
            } = definition
            {
                let variables = self.theme_fragment(tokens, frames, *origin, true, false)?;
                let frames = self.theme_fragment(tokens, frames, *origin, false, true)?;
                theme_variables.append(&variables);
                keyframes.append(&frames);
            }
        }
        // Every rule of a utility that defines keyframes contributes them again, so
        // equal definitions are emitted once.
        let mut emitted = BTreeSet::new();
        for frames in &utility_css.keyframes {
            if emitted.insert(frames.css.as_str()) {
                keyframes.append(frames);
            }
        }
        Ok(SupportOutput {
            theme_variables,
            keyframes,
            properties: self.properties,
        })
    }

    /// Selects one theme block's live declarations and requested animation definitions.
    ///
    /// `tokens` contains indices into `self.theme.values()`, in definition order. The
    /// two flags select declaration and `@keyframes` output independently. Declarations
    /// retain their token origins; selected frames use the block's `origin`. The result
    /// has no selector or layer wrapper. Runtime conditionals use `self.options`;
    /// lowered token declarations map to the original value span rather than separate
    /// name/value spans. Unselected or out-of-range indices and unresolved theme
    /// references are omitted. Declarations precede keyframes when both output flags
    /// are set.
    ///
    /// # Errors
    ///
    /// Returns conditional-lowering errors for selected token values or emitted keyframes.
    /// Token values are validated even when only keyframes are requested. No partial
    /// mapped output is returned and the selection is unchanged.
    fn theme_fragment(
        &self,
        tokens: &[usize],
        keyframes: &str,
        origin: Option<Span>,
        emit_tokens: bool,
        emit_keyframes: bool,
    ) -> Result<MappedStylesheet, CompileDiagnostic> {
        let mut output = MappedStylesheet::default();
        let mut declarations = MappedStylesheet::default();
        for index in tokens.iter().filter(|index| self.tokens.contains(index)) {
            if let Some(token) = self.theme.values().get(*index) {
                let start = declarations.css.len();
                let Some(value) = resolve_theme_value(self.theme, &token.value.value)? else {
                    continue;
                };
                if value.contains("--if(") {
                    let declaration =
                        format_args!("{}:{value};", token.name.value).to_lean_string();
                    let lowered = lower(&declaration, self.options.use_css_if, self.identifiers)?;
                    declarations.text(
                        &self.options.rewrite_generated(&lowered.css),
                        resolve_origin(self.sources, Some(token.value.span)),
                    );
                    declarations.banners.extend(
                        token
                            .banners
                            .iter()
                            .map(|banner| (start..declarations.css.len(), banner.clone())),
                    );
                    continue;
                }
                let fallback = self.theme.color_fallback(
                    &format_args!("{}:{value};", token.name.value).to_lean_string(),
                );
                if let Some(fallback) = &fallback {
                    declarations.text(
                        fallback,
                        resolve_origin(self.sources, Some(token.value.span)),
                    );
                    declarations.text("@supports (color:color-mix(in lab,red,red)){", None);
                }
                declarations.text(
                    &token.name.value,
                    resolve_origin(self.sources, Some(token.name.span)),
                );
                declarations.text(":", None);
                declarations.text(&value, resolve_origin(self.sources, Some(token.value.span)));
                declarations.text(";", None);
                if fallback.is_some() {
                    declarations.text("}", None);
                }
                declarations.banners.extend(
                    token
                        .banners
                        .iter()
                        .map(|banner| (start..declarations.css.len(), banner.clone())),
                );
            }
        }
        if emit_tokens && !declarations.css.is_empty() {
            output.append(&declarations);
        }
        if emit_keyframes {
            output.text(
                &self.options.rewrite_generated(
                    &lower(
                        &extract_authored_keyframes(keyframes, &self.animations),
                        self.options.use_css_if,
                        self.identifiers,
                    )?
                    .css,
                ),
                resolve_origin(self.sources, origin),
            );
        }
        Ok(output)
    }
}

fn resolve_theme_value(
    theme: &ThemeIndex,
    value: &str,
) -> Result<Option<LeanString>, CompileDiagnostic> {
    theme
        .resolve_theme_references(value)
        .map(|value| value.map(|value| theme.inline_asset_references(&value).unwrap_or(value)))
        .map_err(|error| CompileDiagnostic::InvalidThemeReference {
            offset: error.offset(),
            detail: error.to_lean_str(),
        })
}

/// Ready-to-insert support fragments; selectors and conditions belong to the template.
pub(super) struct SupportOutput {
    /// Mapped declarations without selector braces, in template theme-block order
    /// and then each block's token-index order.
    pub(super) theme_variables: MappedStylesheet,
    /// Mapped theme keyframes in block order and then requested-animation order,
    /// followed by producer keyframes in contribution order; duplicates remain.
    pub(super) keyframes: MappedStylesheet,
    /// Registrations and fallback declarations in registry order.
    pub(super) properties: PropertyOutput,
}

/// Appends theme-node references in depth-first source order through group and style children.
///
/// Existing `output` entries remain before the appended nodes. The input tree is
/// unchanged; traversal neither retains surrounding wrappers nor deduplicates nodes.
fn collect_theme_nodes<'a>(nodes: &'a [CssOutputNode], output: &mut Vec<&'a CssOutputNode>) {
    for node in nodes {
        match node {
            CssOutputNode::Group { children, .. } | CssOutputNode::Style { children, .. } => {
                collect_theme_nodes(children, output);
            }
            CssOutputNode::Theme { .. } => {
                output.push(node);
            }
            _ => {}
        }
    }
}
