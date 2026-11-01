//! Lowering of `style.css()` calls into stylesheet strings.
//!
//! A call resolves its utilities as one document-level style, so each utility needs a
//! variant that selects its element, or the call's `selector` option. The rules and the
//! theme values, keyframes, and property support that they require are placed into the
//! named output template that the call selects. The call contributes nothing to the
//! document stylesheet; the string is the whole output.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::CssCall;
use stillwind_source::ModuleId;

use super::RuleBoundaries;
use super::conditionals::{lower, report_skipped_conditionals};
use super::document::{DocumentInput, emit_document_rules};
use super::style_variants::collect_variant_and_applications;
use super::stylesheet::{prefix_theme_variables, render_css_template, rewrite_generated_css};
use crate::identifiers::IdentifierContext;
use crate::{CompilerOptions, MappedStylesheet, finalize_stylesheet};

/// Name of the template that a call without a `template` option selects.
const DEFAULT_TEMPLATE: &str = "default";

/// The part of a call's output that [`css_call_text`] returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CssOutput {
    /// The whole string that the call lowers to.
    Template,
    /// Only the rules of the utilities, without the template and its support.
    Rules,
}

/// Returns the stylesheet text that `call` lowers to, or only its rules when `output`
/// is [`CssOutput::Rules`].
///
/// A group key that does not resolve omits the literals beneath it. A template name
/// that the stylesheet does not define reports a finding, and the call then returns
/// its rules without theme values, keyframes, or property support. The returned text
/// uses rewritten generated names, so callers must not rewrite it again.
///
/// `module_id` identifies the module of `call`, and it keeps the support queries of
/// conditional rules distinct between calls.
///
/// Fatal problems, such as malformed utility definitions, are appended to
/// `diagnostics`, and the result is then empty. A declaration whose runtime condition
/// cannot be lowered is left out and reported at the call.
#[allow(clippy::too_many_arguments)] // One borrow per compilation-wide destination.
pub(crate) fn css_call_text(
    module_id: &ModuleId,
    call: &CssCall,
    theme: &ThemeIndex,
    options: &CompilerOptions,
    identifiers: &IdentifierContext,
    output: CssOutput,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> LeanString {
    let lint_settings = &options.lint_settings();
    let mut prefixes = Vec::with_capacity(call.inputs.len());
    for input in &call.inputs {
        let resolved = input.variants.iter().all(|variant| {
            collect_variant_and_applications(theme, variant, lint_findings).is_some()
        });
        prefixes.push(resolved.then(|| {
            input
                .variants
                .iter()
                .map(|variant| variant.value.as_str())
                .collect::<LeanString>()
        }));
    }
    let inputs = call
        .inputs
        .iter()
        .zip(&prefixes)
        .filter_map(|(input, prefix)| {
            Some(DocumentInput {
                prefix: prefix.as_deref()?,
                utilities: &input.utilities,
            })
        })
        .collect::<Vec<_>>();
    let start = diagnostics.len();
    let rendered = emit_document_rules(
        &inputs,
        call.selector
            .as_ref()
            .map(|selector| selector.value.as_str()),
        theme,
        lint_settings,
        options.use_css_if,
        identifiers,
        diagnostics,
        lint_findings,
    )
    .and_then(|contributions| {
        let mut utilities = MappedStylesheet::default();
        let owner = format_args!(
            "css\0{module_id}\0{}\0{}",
            options.class_namespace, call.span.start.0
        )
        .to_lean_string();
        let mut boundaries = RuleBoundaries::new(owner, identifiers);
        for contribution in contributions {
            let start = utilities.css.len();
            let css = boundaries.render(&contribution.css)?;
            let lowered = lower(&css, options.use_css_if, identifiers)?;
            report_skipped_conditionals(lowered.skipped, call.span, lint_settings, lint_findings);
            utilities
                .css
                .push_str(&rewrite_generated_css(&lowered.css, theme, options));
            utilities.banners.extend(
                contribution
                    .banners
                    .iter()
                    .map(|banner| (start..utilities.css.len(), banner.clone())),
            );
            utilities.required_properties.extend(
                contribution
                    .required_properties
                    .iter()
                    .map(|name| options.rewrite_generated(name)),
            );
        }
        if output == CssOutput::Rules {
            prefix_theme_variables(&mut utilities, theme, options);
            return Ok(finalize_stylesheet(utilities).css.into());
        }
        let name = call
            .template
            .as_ref()
            .map_or(DEFAULT_TEMPLATE, |template| template.value.as_str());
        let Some(template) = theme.css_templates().get(name) else {
            lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidCssOption {
                    reason: format_args!("the stylesheet defines no template `{name}`")
                        .to_lean_str(),
                },
                call.template
                    .as_ref()
                    .map_or(call.span, |template| template.span),
            ));
            return Ok(finalize_stylesheet(utilities).css.into());
        };
        render_css_template(template, theme, &utilities, options, identifiers)
            .map(|output| LeanStr::from(finalize_stylesheet(output).css))
    });
    match rendered {
        Ok(css) if diagnostics.len() == start => css.to_lean_string(),
        Ok(_) => LeanString::new(),
        Err(diagnostic) => {
            diagnostics.push(CompileError::new(diagnostic, call.span));
            LeanString::new()
        }
    }
}
