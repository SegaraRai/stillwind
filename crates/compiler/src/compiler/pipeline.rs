//! Pass order for one module.
//!
//! The pipeline clones the IR, orders style calls by composition topology, allocates a
//! symbol for every declaration, then runs one [`CallEmitter`] per call in that order.
//! Exported or referenced motion definitions are prepended to the emitted utility CSS.
//! Source edits and helper imports are planned last, and prefix rewriting happens after
//! all generated text exists. Successful calls retain direct contribution records;
//! local composition checks run after emission, and hosts later resolve imported bases.
//! Rewriting applies to synthetic fragments while authored expression text and its
//! original source mappings remain unchanged.
//!
//! Failure is all-or-nothing: once any diagnostic exists, the transforms, classes,
//! symbols, and CSS are cleared, so a caller never receives a plan that describes half
//! a compilation.

use std::collections::BTreeSet;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, finish_lint_findings};
use stillwind_ir::{
    ModuleAnalysis, composition_dependencies, ordered_style_calls, validate_composition,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceFile, Span};

use super::conditionals::lower;
use super::definitions::{emit_keyframes_definition, emit_view_transition_definition};
use super::emission::{
    CLASS_KEY, CallEmitter, CallEmitterInputs, KEYFRAMES_KEY, VIEW_TRANSITION_CLASS_KEY,
};
use super::fragment::{render_cva_value, render_style_call};
use super::output::CompileResult;
use super::symbols::{
    detached_symbol, generated_ref_marker_symbol, generated_ref_symbol, generated_symbol,
    named_declaration, ref_marker_name,
};
use super::symbols::{named_identity, ref_identity};
use super::{Compiler, CssContribution};
use crate::editor_lint::detect;
use crate::identifiers::IdentifierContext;
use crate::options::validate_lint_settings;
use crate::runtime::{RuntimeBindings, RuntimeHelper, encode_projected_edits};
use crate::transforms::SourceTransform;
use crate::{
    CompilerOptions, DesignSystemQuery, GeneratedSymbol, GeneratedSymbolKind, StyleResolver,
    SymbolOrigin, check_style,
};

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
    /// Support modules are analyzed like any other module, but their transforms are
    /// dropped: they are not compiled into a file anyone applies edits to.
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

    /// Runs the shared pass sequence, returning emitted artifacts and accumulated
    /// findings. `emit_source` selects whether source replacements and helper imports
    /// are retained.
    ///
    /// `parsed.ir` is cloned before ordering, because the call order is a property of
    /// the compilation, not of the frontend's output, and the analysis may be reused.
    /// `snapshot` supplies the immutable utility and variant registry. `identifiers`
    /// shares the naming context and can accumulate rejected names during emission.
    /// Successful output retains direct effects and locally resolved composition
    /// findings. `compiler_options` selects conditional lowering after rule
    /// construction and before generated-prefix rewriting. Lowering errors join the
    /// diagnostics and clear generated CSS, classes, symbols, transforms, and direct
    /// effects transactionally. `source` must be the original editable buffer described
    /// by `parsed`; source expression mappings retain that coordinate space through
    /// prefix and attribute encoding.
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
            .filter(|identity| is_definition_kind(identity.kind()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut result = CompileResult {
            composition: composition_dependencies(parsed),
            diagnostics: parsed.diagnostics.clone(),
            lint_findings: parsed.lint_findings.clone(),
            ..CompileResult::default()
        };
        let theme = snapshot.theme();
        let module_id = &parsed.module_id;
        let options = compiler_options;
        result.lint_settings = compiler_options.lint_settings();
        if let Err(kind) = validate_lint_settings(&result.lint_settings) {
            result
                .diagnostics
                .push(CompileError::new(kind, source.full_span()));
            finish_lint_findings(
                source,
                &result.lint_settings,
                &mut result.lint_findings,
                false,
            );
            return result;
        }
        let query = DesignSystemQuery::new(snapshot.clone(), compiler_options.clone());
        result.lint_findings.extend(detect(source, parsed, &query));
        result
            .diagnostics
            .extend(validate_composition(&[parsed], source.full_span(), false));

        if !result.diagnostics.is_empty() {
            finish_lint_findings(
                source,
                &result.lint_settings,
                &mut result.lint_findings,
                false,
            );
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
                .keyframes_declarations
                .iter()
                .map(|definition| {
                    generated_symbol(
                        identifiers,
                        module_id,
                        AssetKind::Keyframes,
                        &definition.binding,
                        definition.label.as_ref(),
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
                        module_id,
                        AssetKind::ViewTransition,
                        &definition.binding,
                        definition.label.as_ref(),
                    )
                })
                .filter_map(|symbol| push_or_report(&mut diagnostics, symbol, source.full_span())),
        );
        let mut cva_values = module
            .cva_calls
            .iter()
            .flat_map(|cva| cva.values.iter())
            .collect::<Vec<_>>();
        cva_values.sort_by_key(|call| call.span.start);
        let mut cva_values = cva_values.into_iter().peekable();
        let mut calls = Vec::new();
        for call in &module.style_calls {
            while cva_values
                .peek()
                .is_some_and(|value| value.span.start < call.span.start)
            {
                if let Some(value) = cva_values.next() {
                    calls.push((value, true));
                }
            }
            calls.push((call, false));
        }
        calls.extend(cva_values.map(|value| (value, true)));
        for (call, cva) in calls {
            let emitter_inputs = CallEmitterInputs {
                preview: None,
                source,
                module_id,
                options,
                theme,
                call,
                identifiers,
                diagnostics: &mut diagnostics,
                lint_findings: &mut result.lint_findings,
                utility_css: &mut utility_css,
                symbols: &mut symbols,
                emitted_utility_classes: &mut emitted_utility_classes,
                runtime_assets: &mut runtime_assets,
                required_definitions: &mut required_definitions,
            };
            let Some(emission) = (if cva {
                CallEmitter::run_cva(emitter_inputs)
            } else {
                CallEmitter::run(emitter_inputs)
            }) else {
                continue;
            };
            result.style_effects.push(emission.style_effects.clone());
            result.classes.extend(emission.classes.iter().cloned());
            symbols.extend(emission.written_name_symbols.iter().cloned());
            let replacement = if cva {
                render_cva_value(&emission, module.adapter.spec(), &mut runtime)
            } else {
                render_style_call(&emission, module.adapter.spec(), &mut runtime)
            };
            result.transforms.push(SourceTransform {
                call_span: call.span,
                replacement: replacement.text,
                expressions: replacement.mappings,
            });
        }
        for cva in &module.cva_calls {
            let helper = runtime.require(RuntimeHelper::Cva, cva.callee_span);
            result.transforms.push(SourceTransform {
                call_span: cva.callee_span,
                replacement: helper,
                expressions: Vec::new(),
            });
        }
        let mut definition_css = Vec::new();
        for animation in &module.keyframes_declarations {
            let mut contribution = CssContribution::default();
            emit_keyframes_definition(
                &mut contribution,
                module_id,
                identifiers,
                animation,
                theme,
                &mut diagnostics,
            );
            if required_definitions.contains(&named_identity(
                module_id,
                AssetKind::Keyframes,
                &animation.binding,
            )) {
                definition_css.push(contribution);
            }
        }
        for transition in &module.view_transition_declarations {
            let mut contribution = CssContribution::default();
            emit_view_transition_definition(
                &mut contribution,
                module_id,
                identifiers,
                transition,
                theme,
                &mut diagnostics,
            );
            if required_definitions.contains(&named_identity(
                module_id,
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
        for animation in module.keyframes_declarations {
            result.transforms.push(SourceTransform {
                call_span: animation.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    module_id,
                    AssetKind::Keyframes,
                    &animation.binding,
                )) {
                    match identifiers.keyframes_name(&named_declaration(
                        module_id,
                        &animation.binding,
                        animation.label.as_ref(),
                    )) {
                        Ok(name) => {
                            format!("{{{KEYFRAMES_KEY}:{}}}", name.javascript_string())
                        }
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
                    module_id,
                    AssetKind::ViewTransition,
                    &transition.binding,
                )) {
                    match identifiers.view_transition_class(&named_declaration(
                        module_id,
                        &transition.binding,
                        transition.label.as_ref(),
                    )) {
                        Ok(class) => format!(
                            "{{{VIEW_TRANSITION_CLASS_KEY}:{class}}}",
                            class = class.javascript_string()
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

        // Names that belong to no declaration, such as conditional helpers, are reported
        // after the declaration symbols so a consumer looking for a position finds one.
        // A failed compilation reports nothing, like the cleared symbol list above.
        if result.diagnostics.is_empty() {
            result.symbols.extend(
                identifiers
                    .drain_detached()
                    .iter()
                    .map(|name| detached_symbol(name, SymbolOrigin::Module(module_id.clone()))),
            );
        }
        for transform in &mut result.transforms {
            transform.rewrite_generated(|text| options.rewrite_generated(text));
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name).into();
        }

        result
            .source_lint_findings
            .clone_from(&result.lint_findings);
        if result.diagnostics.is_empty() {
            let mut resolver = StyleResolver::new(&result.style_effects);
            for effects in &result.style_effects {
                if let Some(style) = resolver.resolve(effects.identity()) {
                    result.lint_findings.extend(check_style(&style));
                }
            }
        } else {
            result.style_effects.clear();
        }
        finish_lint_findings(
            source,
            &result.lint_settings,
            &mut result.lint_findings,
            result.diagnostics.is_empty(),
        );

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
    let mut seen = BTreeSet::new();
    symbols
        .into_iter()
        .filter(|symbol| {
            if symbol.kind == GeneratedSymbolKind::Class {
                return true;
            }
            seen.insert((symbol.kind, symbol.name.clone(), symbol.identity.clone()))
        })
        .collect()
}

/// One ref declaration's runtime value: `{ c: "marker-class" }`.
///
/// A declaration is an identity, and every attachment reads that one key, so a ref
/// needs no runtime helper and no projection helper functions. The key is a plain
/// property read, which is what makes `style(ref)` and the projections static.
///
/// `token` is the marker written as a JavaScript string literal. Attachments and
/// projections read `c` directly as their class contribution.
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
    value: Result<T, CompileDiagnostic>,
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

/// Whether the CSS of one asset kind belongs to this band.
fn is_definition_kind(kind: AssetKind) -> bool {
    matches!(kind, AssetKind::Keyframes | AssetKind::ViewTransition)
}
