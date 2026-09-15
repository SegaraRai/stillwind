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
use stillwind_diagnostics::{CompileError, filter_lint_findings};
use stillwind_ir::{
    ModuleAnalysis, composition_dependencies, ordered_style_calls, validate_composition,
};
use stillwind_semantic::AssetKind;
use stillwind_source::SourceFile;

use super::conditionals::lower;
use super::emission::{CallEmitter, CallEmitterInputs};
use super::fragment::render_style_call;
use super::handles::{named_identity, ref_identity, scope_identity, summary_runtime_assets};
use super::motion::{emit_animation_definition, emit_view_transition_definition};
use super::output::CompileResult;
use super::symbols::{
    generated_ref_marker_symbol, generated_ref_symbol, generated_scope_marker_symbol,
    generated_scope_symbol, generated_symbol, motion_name, ref_marker_name, scope_marker_name,
};
use super::{Compiler, CssContribution};
use crate::CompilerOptions;
use crate::GeneratedSymbol;
use crate::generated::js_string;
use crate::identifier::IdentifierContext;
use crate::runtime::{RuntimeBindings, RuntimeHelper, encode_projected_edits};
use crate::transforms::SourceTransform;

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
        let mut runtime_assets = summary_runtime_assets(&parsed.summary);
        let mut motion_assets = runtime_assets.clone();
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
        let mut runtime = RuntimeBindings::new(parsed);
        let mut symbols = std::mem::take(&mut result.symbols);
        symbols.extend(
            module
                .handle_definitions
                .iter()
                .map(|definition| generated_ref_symbol(identifiers, definition))
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        symbols.extend(
            module
                .handle_definitions
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
                    generated_symbol(identifiers, source, AssetKind::Animation, &definition.name)
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
                        &definition.name,
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
                motion_assets: &mut motion_assets,
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
        motion_assets.extend(runtime_assets.iter().cloned());
        let mut motion_css = Vec::new();
        for animation in &module.animation_declarations {
            let mut definition_css = CssContribution::default();
            emit_animation_definition(
                &mut definition_css,
                source,
                identifiers,
                animation,
                theme,
                &mut diagnostics,
            );
            if motion_assets.contains(&named_identity(
                source,
                AssetKind::Animation,
                &animation.name,
            )) {
                motion_css.push(definition_css);
            }
        }
        for transition in &module.view_transition_declarations {
            let mut definition_css = CssContribution::default();
            emit_view_transition_definition(
                &mut definition_css,
                source,
                identifiers,
                transition,
                theme,
                &mut diagnostics,
            );
            if motion_assets.contains(&named_identity(
                source,
                AssetKind::ViewTransition,
                &transition.name,
            )) {
                motion_css.push(definition_css);
            }
        }
        motion_css.append(&mut utility_css);
        utility_css = motion_css;
        for handle_definition in module.handle_definitions {
            result.transforms.push(SourceTransform {
                call_span: handle_definition.call_span,
                replacement: if runtime_assets.contains(&ref_identity(&handle_definition)) {
                    match ref_marker_name(identifiers, &handle_definition) {
                        Ok(marker) => handle_value(&marker),
                        Err(failure) => {
                            diagnostics
                                .push(CompileError::new(failure, handle_definition.call_span));
                            "undefined".to_owned()
                        }
                    }
                } else {
                    "undefined".to_owned()
                },
                expressions: Vec::new(),
            });
        }
        for scope_definition in module.scope_definitions {
            result.transforms.push(SourceTransform {
                call_span: scope_definition.call_span,
                replacement: if runtime_assets.contains(&scope_identity(&scope_definition)) {
                    match scope_marker_name(identifiers, &scope_definition) {
                        Ok(marker) => handle_value(&marker),
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
                    &animation.name,
                )) {
                    match motion_name(
                        identifiers,
                        source.path(),
                        AssetKind::Animation,
                        &animation.name,
                    ) {
                        Ok(name) => format!(
                            "{}(\"{name}\")",
                            runtime.require(RuntimeHelper::Animation, animation.call_span)
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
                    &transition.name,
                )) {
                    match motion_name(
                        identifiers,
                        source.path(),
                        AssetKind::ViewTransition,
                        &transition.name,
                    ) {
                        Ok(name) => format!(
                            "{}(\"{name}\")",
                            runtime.require(RuntimeHelper::ViewTransition, transition.call_span)
                        ),
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
                match lower(&contribution.css, options.use_css_if) {
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

        for transform in &mut result.transforms {
            transform.rewrite_generated(|text| options.rewrite_generated(text));
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name);
        }

        filter_lint_findings(source, &mut result.lint_findings);

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

/// Keeps a named symbol, or reports the generator's failure at `span`.
/// One ref or scope declaration's runtime value: its canonical class token under `c`.
///
/// A declaration is an identity, and every attachment reads that one key, so a handle
/// needs no runtime helper and no projection helper functions. The key is a plain
/// property read, which is what makes `style(ref)` and the projections static.
fn handle_value(marker: &str) -> String {
    format!("{{c:{}}}", js_string(marker))
}

/// Keeps a named symbol, or reports the generator's failure at `span`.
///
/// A rejected name means the declaration has no generated identifier, so the symbol is
/// dropped; the pipeline clears every artifact once a diagnostic exists, so no consumer
/// sees a partially named module.
fn push_or_report<T>(
    diagnostics: &mut Vec<CompileError>,
    value: Result<T, stillwind_diagnostics::CompileErrorKind>,
    span: stillwind_source::Span,
) -> Option<T> {
    match value {
        Ok(value) => Some(value),
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, span));
            None
        }
    }
}
