//! Pass order for one module.
//!
//! The pipeline clones the IR, orders style calls by composition topology, allocates a
//! symbol for every declaration, then runs one [`CallEmitter`] per call in that order.
//! Exported or referenced motion definitions are prepended to the emitted utility CSS.
//! Source edits and helper imports are planned last, and prefix rewriting happens after
//! all generated text exists. Rewriting applies to synthetic fragments while authored
//! expression text and its original source mappings remain unchanged.
//!
//! Failure is all-or-nothing: once any diagnostic exists, the transforms, classes, symbols,
//! and CSS are cleared, so a caller never receives a plan that describes half a compilation.

use std::collections::BTreeSet;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileError, CompileErrorKind, filter_lint_findings};
use stillwind_ir::{
    ModuleAnalysis, ViewTransitionKey, composition_dependencies, ordered_style_calls,
    split_view_transition_classes, validate_composition,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceFile, Span};

use super::conditionals::lower;
use super::definitions::{
    emit_animation_definition, emit_view_transition_definition, is_definition_kind,
};
use super::emission::{CLASS_KEY, CallEmitter, CallEmitterInputs};
use super::fragment::render_style_call;
use super::output::CompileResult;
use super::symbols::{
    detached_symbol, generated_ref_marker_symbol, generated_ref_symbol,
    generated_scope_marker_symbol, generated_scope_symbol, generated_symbol, identity_declaration,
    named_declaration, ref_marker_name, scope_marker_name,
};
use super::symbols::{named_identity, ref_identity, scope_identity};
use super::{Compiler, CssContribution};
use crate::generated::js_string;
use crate::identifiers::IdentifierContext;
use crate::runtime::{RuntimeBindings, RuntimeHelper, encode_projected_edits};
use crate::transforms::SourceTransform;
use crate::{CompilerOptions, GeneratedSymbol};

impl Compiler {
    /// Compiles a module that owns source edits, including its runtime imports.
    pub(crate) fn compile_with_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &CompilerOptions,
        identifiers: &IdentifierContext,
    ) -> CompileResult {
        Self::compile_snapshot(
            source,
            parsed,
            snapshot,
            compiler_options,
            identifiers,
            true,
        )
    }

    /// Compiles a module whose only role is to supply support declarations.
    ///
    /// Support modules are analyzed like any other module, but their transforms are dropped:
    /// they are not compiled into a file anyone applies edits to.
    pub(crate) fn compile_support_with_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &CompilerOptions,
        identifiers: &IdentifierContext,
    ) -> CompileResult {
        Self::compile_snapshot(
            source,
            parsed,
            snapshot,
            compiler_options,
            identifiers,
            false,
        )
    }

    /// Runs the shared pass sequence, returning emitted artifacts and accumulated findings.
    /// `emit_source` selects whether source replacements and helper imports are retained.
    ///
    /// `parsed.ir` is cloned before ordering, because the call order is a property of the
    /// compilation, not of the frontend's output, and the analysis may be reused.
    /// `compiler_options` selects conditional lowering after rule construction and
    /// before generated-prefix rewriting. Lowering errors join the diagnostics and
    /// clear generated CSS, classes, symbols, and transforms transactionally.
    /// `source` must be the original editable buffer described by `parsed`; source
    /// expression mappings retain that coordinate space through prefix and attribute encoding.
    #[allow(clippy::too_many_lines)]
    fn compile_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &CompilerOptions,
        identifiers: &IdentifierContext,
        emit_source: bool,
    ) -> CompileResult {
        let mut runtime_assets = parsed.summary.export_reachable_identities();
        let mut required_definitions = runtime_assets
            .iter()
            .filter(|identity| is_definition_kind(identity.kind))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut result = CompileResult {
            composition: composition_dependencies(parsed),
            diagnostics: parsed.diagnostics.clone(),
            lint_findings: parsed.lint_findings.clone(),
            ..CompileResult::default()
        };
        let theme = snapshot.theme();
        let options = compiler_options;
        result.lint_policy.clone_from(&theme.config.lint_rules);
        filter_lint_findings(source, &mut result.lint_findings);
        result
            .lint_findings
            .extend(split_view_transition_classes(parsed));
        result
            .diagnostics
            .extend(validate_composition(&[parsed], source.full_span(), false));

        if !result.diagnostics.is_empty() {
            return result;
        }

        let mut diagnostics = std::mem::take(&mut result.diagnostics);
        let mut utility_css = Vec::new();
        let mut emitted_utility_classes = BTreeSet::new();
        let mut module = parsed.ir.clone();
        module.style_calls = ordered_style_calls(parsed);
        let mut runtime = RuntimeBindings::new(parsed).in_mode(identifiers.mode());
        let mut symbols = std::mem::take(&mut result.symbols);
        symbols.extend(
            module
                .ref_definitions
                .iter()
                .map(|definition| generated_ref_symbol(identifiers, definition))
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .ref_definitions
                .iter()
                .map(|definition| generated_ref_marker_symbol(identifiers, definition))
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .scope_definitions
                .iter()
                .map(|definition| generated_scope_symbol(identifiers, definition))
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .scope_definitions
                .iter()
                .map(|definition| generated_scope_marker_symbol(identifiers, definition))
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .animation_declarations
                .iter()
                .map(|definition| {
                    generated_symbol(
                        identifiers,
                        source,
                        AssetKind::Animation,
                        &definition.binding,
                    )
                })
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .view_transition_declarations
                .iter()
                .map(|definition| {
                    generated_symbol(
                        identifiers,
                        source,
                        AssetKind::ViewTransition,
                        &definition.binding,
                    )
                })
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        for call in module.style_calls {
            let Some(emission) = CallEmitter::run(CallEmitterInputs {
                source,
                options,
                theme,
                call: &call,
                identifiers,
                diagnostics: &mut diagnostics,
                lint_findings: &mut result.lint_findings,
                utility_css: &mut utility_css,
                symbols: &mut symbols,
                emitted_utility_classes: &mut emitted_utility_classes,
                runtime_assets: &mut runtime_assets,
                required_definitions: &mut required_definitions,
            }) else {
                continue;
            };
            result.classes.extend(emission.classes.iter().cloned());
            symbols.extend(emission.written_name_symbols.iter().cloned());
            let replacement = render_style_call(&emission, module.adapter.spec(), &mut runtime);
            result.transforms.push(SourceTransform {
                call_span: call.span,
                replacement: replacement.text,
                expressions: replacement.mappings,
            });
        }
        for application in &module.view_transition_key_applications {
            for transition in &application.transitions {
                runtime_assets.insert(transition.identity.clone());
                required_definitions.insert(transition.identity.clone());
            }
        }
        let mut definition_css = Vec::new();
        for animation in &module.animation_declarations {
            let mut contribution = CssContribution::default();
            emit_animation_definition(
                &mut contribution,
                source,
                identifiers,
                animation,
                theme,
                &mut diagnostics,
            );
            if required_definitions.contains(&named_identity(
                source,
                AssetKind::Animation,
                &animation.binding,
            )) {
                definition_css.push(contribution);
            }
        }
        for transition in &module.view_transition_declarations {
            let mut contribution = CssContribution::default();
            emit_view_transition_definition(
                &mut contribution,
                source,
                identifiers,
                transition,
                theme,
                &mut diagnostics,
            );
            if required_definitions.contains(&named_identity(
                source,
                AssetKind::ViewTransition,
                &transition.binding,
            )) {
                definition_css.push(contribution);
            }
        }
        definition_css.append(&mut utility_css);
        utility_css = definition_css;
        for ref_definition in module.ref_definitions {
            result.transforms.push(SourceTransform {
                call_span: ref_definition.call_span,
                replacement: if runtime_assets.contains(&ref_identity(&ref_definition)) {
                    match ref_marker_name(identifiers, &ref_definition) {
                        Ok(marker) => ref_value(&marker.javascript_string()),
                        Err(failure) => {
                            diagnostics.push(CompileError::new(failure, ref_definition.call_span));
                            "undefined".to_owned()
                        }
                    }
                } else {
                    "undefined".to_owned()
                },
                expressions: Vec::new(),
            });
        }
        for application in &module.view_transition_key_applications {
            let key = match &application.key {
                ViewTransitionKey::Literal(key) => js_string(key),
                ViewTransitionKey::Dynamic(span) => {
                    if let Ok(text) = source.slice(*span) {
                        text.to_owned()
                    } else {
                        diagnostics.push(CompileError::new(
                            CompileErrorKind::InvalidViewTransitionKeyExpression,
                            application.call_span,
                        ));
                        continue;
                    }
                }
            };
            let replacement = match (&application.key, application.transitions.as_slice()) {
                (ViewTransitionKey::Literal(_), [transition]) => match identifiers
                    .view_transition_class(&identity_declaration(&transition.identity))
                {
                    Ok(class) => format!("{{v:{},k:{key}}}", class.javascript_string()),
                    Err(failure) => {
                        diagnostics.push(CompileError::new(failure, application.call_span));
                        "undefined".to_owned()
                    }
                },
                _ => {
                    if let Ok(receiver) = source.slice(application.value) {
                        format!(
                            "{}({receiver},{key})",
                            runtime
                                .require(RuntimeHelper::ViewTransitionKey, application.call_span)
                        )
                    } else {
                        diagnostics.push(CompileError::new(
                            CompileErrorKind::InvalidViewTransitionKeyExpression,
                            application.call_span,
                        ));
                        continue;
                    }
                }
            };
            result.transforms.push(SourceTransform {
                call_span: application.call_span,
                replacement,
                expressions: Vec::new(),
            });
        }
        for scope_definition in module.scope_definitions {
            result.transforms.push(SourceTransform {
                call_span: scope_definition.call_span,
                replacement: if runtime_assets.contains(&scope_identity(&scope_definition)) {
                    match scope_marker_name(identifiers, &scope_definition) {
                        Ok(marker) => ref_value(&marker.javascript_string()),
                        Err(failure) => {
                            diagnostics
                                .push(CompileError::new(failure, scope_definition.call_span));
                            "undefined".to_owned()
                        }
                    }
                } else {
                    "undefined".to_owned()
                },
                expressions: Vec::new(),
            });
        }
        for animation in module.animation_declarations {
            result.transforms.push(SourceTransform {
                call_span: animation.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::Animation,
                    &animation.binding,
                )) {
                    match identifiers
                        .animation_name(&named_declaration(source.path(), &animation.binding))
                    {
                        Ok(name) => format!(
                            "{}({})",
                            runtime.require(RuntimeHelper::Animation, animation.call_span),
                            name.javascript_string()
                        ),
                        Err(failure) => {
                            diagnostics.push(CompileError::new(failure, animation.call_span));
                            "undefined".to_owned()
                        }
                    }
                } else {
                    "undefined".to_owned()
                },
                expressions: Vec::new(),
            });
        }
        for transition in module.view_transition_declarations {
            result.transforms.push(SourceTransform {
                call_span: transition.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::ViewTransition,
                    &transition.binding,
                )) {
                    match identifiers.view_transition_class(&named_declaration(
                        source.path(),
                        &transition.binding,
                    )) {
                        Ok(class) => format!("{{v:{}}}", class.javascript_string()),
                        Err(failure) => {
                            diagnostics.push(CompileError::new(failure, transition.call_span));
                            "undefined".to_owned()
                        }
                    }
                } else {
                    "undefined".to_owned()
                },
                expressions: Vec::new(),
            });
        }

        result.diagnostics = diagnostics;
        result.symbols = dedupe_name_symbols(symbols);
        if emit_source && result.diagnostics.is_empty() {
            match runtime.plan(parsed, &result.transforms).and_then(|edits| {
                encode_projected_edits(parsed, &mut result.transforms)?;
                Ok(edits)
            }) {
                Ok(edits) => result.transforms.extend(edits),
                Err(error) => result.diagnostics.push(error),
            }
        }

        if !emit_source {
            result.transforms.clear();
        }

        if result.diagnostics.is_empty() {
            for contribution in &mut utility_css {
                match lower(&contribution.css, options.use_css_if, identifiers) {
                    Ok(css) => contribution.css = css,
                    Err(kind) => result
                        .diagnostics
                        .push(CompileError::new(kind, source.full_span())),
                }
            }
        }
        if result.diagnostics.is_empty() {
            for contribution in &mut utility_css {
                contribution.css = options.rewrite_generated(&contribution.css);
                for name in &mut contribution.required_properties {
                    *name = options.rewrite_generated(name);
                }
            }
            result.module_css = utility_css.iter().map(|part| part.css.as_str()).collect();
            result.contributions = utility_css;
        } else {
            result.css.clear();
            result.classes.clear();
            result.symbols.clear();
            result.transforms.clear();
        }

        // Names that belong to no declaration, such as conditional helpers, are reported after the
        // declaration symbols so a consumer looking for a position finds one. A failed compilation
        // reports nothing, like the cleared symbol list above.
        if result.diagnostics.is_empty() {
            result.symbols.extend(
                identifiers
                    .drain_detached()
                    .iter()
                    .map(|name| detached_symbol(name, source.path())),
            );
        }
        for transform in &mut result.transforms {
            transform.rewrite_generated(|text| options.rewrite_generated(text));
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name);
        }

        filter_lint_findings(source, &mut result.lint_findings);

        result
            .rejected_names
            .extend(identifiers.drain_rejected().into_entries());
        result
    }
}

/// Drops repeated records for one written name.
///
/// A declaration's `anchor-name` and `container-name` are written once per projection
/// site, so the same name and identity arrives many times. Class symbols are already
/// allocated once per unit and keep their position; a repeated name symbol keeps its
/// first occurrence, which is the source order of the projection that first used it.
fn dedupe_name_symbols(symbols: Vec<GeneratedSymbol>) -> Vec<GeneratedSymbol> {
    let mut seen = std::collections::BTreeSet::new();
    symbols
        .into_iter()
        .filter(|symbol| {
            if symbol.kind == "class" {
                return true;
            }
            seen.insert((
                symbol.kind.clone(),
                symbol.name.clone(),
                symbol.identity.clone(),
            ))
        })
        .collect()
}

/// One ref or scope declaration's runtime value: its canonical class token under `c`.
///
/// A declaration is an identity, and every attachment reads that one key, so a ref
/// needs no runtime helper and no projection helper functions. The key is a plain
/// property read, which is what makes `style(ref)` and the projections static.
///
/// `token` is the token written as a JavaScript string literal, because generated
/// JavaScript reads the same token back as the projection table's key.
fn ref_value(token: &str) -> String {
    format!("{{{CLASS_KEY}:{token}}}")
}

/// Keeps a named symbol, or reports the generator's failure at `span`.
///
/// A rejected name means the declaration has no generated identifier, so the symbol is
/// dropped; the pipeline clears every artifact once a diagnostic exists, so no consumer
/// sees a partially named module.
fn push_or_report<T>(
    diagnostics: &mut Vec<CompileError>,
    value: Result<T, CompileErrorKind>,
    span: Span,
) -> Option<T> {
    match value {
        Ok(value) => Some(value),
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, span));
            None
        }
    }
}
