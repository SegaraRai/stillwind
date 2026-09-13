//! Pass order for one module.
//!
//! The pipeline clones the IR, orders style calls by composition topology, allocates a
//! symbol for every declaration, then runs one [`CallEmitter`] per call in that order.
//! Exported or referenced motion definitions are prepended to the emitted utility CSS.
//! Source edits and helper imports are planned last, and prefix rewriting happens after
//! all generated text exists, so a configured `--sw-` or `--tw-` spelling is never
//! re-interpreted in the middle of a pass.
//!
//! Failure is all-or-nothing: once any diagnostic exists, the transforms, classes, symbols,
//! and CSS are cleared, so a caller never receives a plan that describes half a compilation.

use std::collections::BTreeSet;

use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{
    ModuleAnalysis, composition_dependencies, ordered_style_calls, validate_composition,
};
use stillwind_semantic::AssetKind;
use stillwind_source::{CompileError, SourceFile, filter_lint_findings};

use super::emission::{CallEmitter, CallEmitterInputs};
use super::handles::{
    handle_marker_class, named_identity, ref_identity, scope_identity, scope_marker_class,
    summary_runtime_assets,
};
use super::motion::{emit_animation_definition, emit_view_transition_definition};
use super::output::CompileResult;
use super::props::render_style_call;
use super::symbols::{
    generated_ref_marker_symbol, generated_ref_symbol, generated_scope_marker_symbol,
    generated_scope_symbol, generated_symbol, symbol_name,
};
use super::{Compiler, CssContribution};
use crate::runtime::{RuntimeBindings, RuntimeHelper, encode_projected_edits};
use crate::transforms::SourceTransform;
use crate::{CompilerOptions, ImportPlanError};

impl Compiler {
    /// Compiles a module that owns source edits, including its runtime imports.
    pub(crate) fn compile_with_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &CompilerOptions,
    ) -> CompileResult {
        Self::compile_snapshot(source, parsed, snapshot, compiler_options, true)
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
    ) -> CompileResult {
        Self::compile_snapshot(source, parsed, snapshot, compiler_options, false)
    }

    /// The shared pass sequence; `emit_source` selects whether edits are returned.
    ///
    /// `parsed.ir` is cloned before ordering, because the call order is a property of the
    /// compilation, not of the frontend's output, and the analysis may be reused.
    #[allow(clippy::too_many_lines)]
    fn compile_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &CompilerOptions,
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

        let mut utility_css = Vec::new();
        let mut emitted_utility_classes = BTreeSet::new();
        let mut module = parsed.ir.clone();
        module.style_calls = ordered_style_calls(parsed);
        let mut runtime = RuntimeBindings::new(parsed);
        result
            .symbols
            .extend(module.handle_definitions.iter().map(generated_ref_symbol));
        result.symbols.extend(
            module
                .handle_definitions
                .iter()
                .map(generated_ref_marker_symbol),
        );
        result
            .symbols
            .extend(module.scope_definitions.iter().map(generated_scope_symbol));
        result.symbols.extend(
            module
                .scope_definitions
                .iter()
                .map(generated_scope_marker_symbol),
        );
        result.symbols.extend(
            module
                .animation_declarations
                .iter()
                .map(|definition| generated_symbol(source, AssetKind::Animation, &definition.name)),
        );
        result.symbols.extend(
            module
                .view_transition_declarations
                .iter()
                .map(|definition| {
                    generated_symbol(source, AssetKind::ViewTransition, &definition.name)
                }),
        );
        for call in module.style_calls {
            let Some(emission) = CallEmitter::run(CallEmitterInputs {
                source,
                options,
                theme,
                call: &call,
                class_property: module.adapter.spec().class_property,
                diagnostics: &mut result.diagnostics,
                lint_findings: &mut result.lint_findings,
                utility_css: &mut utility_css,
                symbols: &mut result.symbols,
                emitted_utility_classes: &mut emitted_utility_classes,
                runtime_assets: &mut runtime_assets,
                motion_assets: &mut motion_assets,
                runtime: &mut runtime,
            }) else {
                continue;
            };
            result.classes.extend(emission.classes.iter().cloned());
            let replacement = render_style_call(&emission, &mut runtime);
            result.transforms.push(SourceTransform {
                call_span: call.span,
                replacement,
            });
        }
        motion_assets.extend(runtime_assets.iter().cloned());
        let mut motion_css = Vec::new();
        for animation in &module.animation_declarations {
            let mut definition_css = CssContribution::default();
            emit_animation_definition(
                &mut definition_css,
                source,
                animation,
                theme,
                &mut result.diagnostics,
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
                transition,
                theme,
                &mut result.diagnostics,
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
                    format!(
                        "{}(\"{}\")",
                        runtime.require(RuntimeHelper::Ref, handle_definition.call_span),
                        handle_marker_class(&handle_definition)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }
        for scope_definition in module.scope_definitions {
            result.transforms.push(SourceTransform {
                call_span: scope_definition.call_span,
                replacement: if runtime_assets.contains(&scope_identity(&scope_definition)) {
                    format!(
                        "{}(\"{}\")",
                        runtime.require(RuntimeHelper::Scope, scope_definition.call_span),
                        scope_marker_class(&scope_definition).trim_start_matches('.')
                    )
                } else {
                    "undefined".to_owned()
                },
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
                    format!(
                        "{}(\"{}\")",
                        runtime.require(RuntimeHelper::Animation, animation.call_span),
                        symbol_name(source, AssetKind::Animation, &animation.name)
                    )
                } else {
                    "undefined".to_owned()
                },
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
                    format!(
                        "{}(\"{}\")",
                        runtime.require(RuntimeHelper::ViewTransition, transition.call_span),
                        symbol_name(source, AssetKind::ViewTransition, &transition.name)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }

        if emit_source && result.diagnostics.is_empty() {
            match runtime.plan(parsed, &result.transforms).and_then(|edits| {
                encode_projected_edits(parsed, &mut result.transforms)?;
                Ok(edits)
            }) {
                Ok(edits) => result.transforms.extend(edits),
                Err(error) => {
                    let span = match &error {
                        ImportPlanError::UntransformedReference { span, .. }
                        | ImportPlanError::UnmappedReference { span, .. }
                        | ImportPlanError::InvalidMetadata { span }
                        | ImportPlanError::MissingModuleCssTarget { span }
                        | ImportPlanError::MissingRuntimeAnchor { span } => *span,
                        ImportPlanError::Source(_) => source.full_span(),
                    };
                    result.diagnostics.push(CompileError::new(
                        "SHIMPORT001",
                        error.to_string(),
                        span,
                    ));
                }
            }
        }

        if !emit_source {
            result.transforms.clear();
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
            transform.replacement = options.rewrite_generated(&transform.replacement);
        }
        for symbol in &mut result.symbols {
            symbol.name = options.rewrite_generated(&symbol.name);
        }

        filter_lint_findings(source, &mut result.lint_findings);

        result
    }
}
