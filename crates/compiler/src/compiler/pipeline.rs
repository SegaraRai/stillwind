use std::collections::BTreeSet;

use stillwind_design_system::{
    DesignSystemSnapshot, compatibility_marker_class, is_tailwind_relationship_candidate,
};
use stillwind_ir::{ModuleAnalysis, StyleInput};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{CompileError, LintFinding, SourceFile, filter_lint_findings};
use stillwind_syntax::parse_candidate;

use super::Compiler;
use super::handles::{
    handle_marker_class, handle_parent_selector, named_identity, ref_identity,
    render_handle_projection, scope_identity, scope_marker_class, summary_runtime_assets,
};
use super::lexer::lex_utilities;
use super::motion::{
    emit_animation_definition, emit_view_transition_definition, resolve_animation_use,
};
use super::output::CompileResult;
use super::react::props::{
    DynamicAssetClassEntry, GeneratedClass, ReactPropsParts, ViewTransitionStyleEntry,
    render_react_props,
};
use super::stylesheet::render::render_stylesheet;
use super::symbols::{
    asset_symbol_name, class_name, generated_class_symbol, generated_ref_marker_symbol,
    generated_ref_symbol, generated_scope_marker_symbol, generated_scope_symbol, generated_symbol,
    handle_projection_name, ref_symbol_name, runtime_projection_class, symbol_name,
};
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_rule, view_transition_utility,
};
use crate::transforms::SourceTransform;

impl Compiler {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_with_snapshot(
        source: &SourceFile,
        parsed: &ModuleAnalysis,
        snapshot: &DesignSystemSnapshot,
        compiler_options: &crate::CompilerOptions,
    ) -> CompileResult {
        let mut runtime_assets = summary_runtime_assets(&parsed.summary);
        let mut result = CompileResult {
            diagnostics: parsed.diagnostics.clone(),
            lint_findings: parsed.lint_findings.clone(),
            ..CompileResult::default()
        };
        let theme = snapshot.theme();
        let options = compiler_options;
        result.lint_policy.clone_from(&theme.config.lint_rules);
        filter_lint_findings(source, &mut result.lint_findings);

        if !result.diagnostics.is_empty() {
            return result;
        }

        let mut utility_css = String::new();
        let mut emitted_utility_classes = BTreeSet::new();
        let module = parsed.ir.clone();
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
        for animation in &module.animation_declarations {
            emit_animation_definition(
                &mut utility_css,
                source,
                animation,
                theme,
                &mut result.diagnostics,
            );
        }
        for transition in &module.view_transition_declarations {
            emit_view_transition_definition(
                &mut utility_css,
                source,
                transition,
                theme,
                &mut result.diagnostics,
            );
        }
        for call in module.style_calls {
            let mut branches = vec![(None, true, String::new(), Vec::new())];
            let mut attributes = Vec::new();
            let mut handle_markers = Vec::new();
            let mut external_classes = Vec::new();
            let mut dynamic_handle_classes = Vec::new();
            let mut dynamic_asset_classes = Vec::new();
            let mut view_transition_style = None;
            let mut compatibility_classes = Vec::new();
            for input in call.inputs {
                let (guard, guard_when_true, selector, input) = match input {
                    StyleInput::ViewTransitionUse { transition, key } => {
                        if view_transition_style.is_some() {
                            result.diagnostics.push(CompileError::new(
                                "SHVIEW004",
                                "style() accepts at most one view transition",
                                call.span,
                            ));
                            continue;
                        }
                        let name = asset_symbol_name(
                            &transition.module_path,
                            AssetKind::ViewTransition,
                            &transition.name,
                        );
                        let branch = &mut branches[0];
                        branch.2.push_str("view-transition");
                        branch.3.push(view_transition_utility());
                        branch.2.push('\0');
                        view_transition_style = Some(ViewTransitionStyleEntry {
                            value: None,
                            identity: Some(name.clone()),
                            candidates: vec![name],
                            key,
                        });
                        continue;
                    }
                    StyleInput::DynamicViewTransitionUse {
                        value,
                        transitions,
                        key,
                    } => {
                        if view_transition_style.is_some() {
                            result.diagnostics.push(CompileError::new(
                                "SHVIEW004",
                                "style() accepts at most one view transition",
                                call.span,
                            ));
                            continue;
                        }
                        let mut candidates = Vec::new();
                        for transition in transitions {
                            runtime_assets.insert(IdentityId::new(
                                AssetKind::ViewTransition,
                                &transition.module_path,
                                transition.name.span.start.0,
                                transition.name.span.end.0,
                                transition.name.value.clone(),
                            ));
                            let name = asset_symbol_name(
                                &transition.module_path,
                                AssetKind::ViewTransition,
                                &transition.name,
                            );
                            candidates.push(name);
                        }
                        let branch = &mut branches[0];
                        branch.2.push_str("view-transition");
                        branch.2.push('\0');
                        branch.3.push(view_transition_utility());
                        view_transition_style = Some(ViewTransitionStyleEntry {
                            value: Some(value),
                            identity: None,
                            candidates,
                            key,
                        });
                        continue;
                    }
                    StyleInput::AnimationUse { animation, options } => {
                        let branch = &mut branches[0];
                        branch.2.push_str("animation:");
                        branch.2.push_str(&animation.name.value);
                        branch.2.push(':');
                        branch.2.push_str(&options.value);
                        branch.2.push('\0');
                        branch.3.push(resolve_animation_use(
                            &animation,
                            &options,
                            theme,
                            &mut result.diagnostics,
                        ));
                        continue;
                    }
                    StyleInput::DynamicAnimationUse {
                        value,
                        animations,
                        options,
                    } => {
                        let mut classes = Vec::new();
                        for animation in animations {
                            runtime_assets.insert(IdentityId::new(
                                AssetKind::Animation,
                                &animation.module_path,
                                animation.name.span.start.0,
                                animation.name.span.end.0,
                                animation.name.value.clone(),
                            ));
                            let identity = asset_symbol_name(
                                &animation.module_path,
                                AssetKind::Animation,
                                &animation.name,
                            );
                            let canonical =
                                format!("dynamic-animation:{identity}:{}", options.value);
                            let canonical = compiler_options.class_input(source.path(), &canonical);
                            let class_name = class_name(&canonical);
                            if emitted_utility_classes
                                .insert((class_name.clone(), canonical.clone()))
                            {
                                emit_utility_rules(
                                    &mut utility_css,
                                    &class_name,
                                    &[resolve_animation_use(
                                        &animation,
                                        &options,
                                        theme,
                                        &mut result.diagnostics,
                                    )],
                                    theme,
                                );
                                result.symbols.push(generated_class_symbol(
                                    source,
                                    &class_name,
                                    &canonical,
                                    call.span,
                                ));
                            }
                            classes.push((identity, class_name));
                        }
                        dynamic_asset_classes.push(DynamicAssetClassEntry { value, classes });
                        continue;
                    }
                    StyleInput::SelectorAnimationUse {
                        selector,
                        animation,
                        options,
                    } => {
                        let branch = &mut branches[0];
                        branch.2.push_str(&selector.value);
                        branch.2.push('\0');
                        branch.2.push_str("animation:");
                        branch.2.push_str(&animation.name.value);
                        branch.2.push(':');
                        branch.2.push_str(&options.value);
                        branch.2.push('\0');
                        let mut utility = resolve_animation_use(
                            &animation,
                            &options,
                            theme,
                            &mut result.diagnostics,
                        );
                        utility.selector = Some(selector.value);
                        branch.3.push(utility);
                        continue;
                    }
                    StyleInput::DynamicSelectorAnimationUse {
                        selector,
                        value,
                        animations,
                        options,
                    } => {
                        let mut classes = Vec::new();
                        for animation in animations {
                            runtime_assets.insert(IdentityId::new(
                                AssetKind::Animation,
                                &animation.module_path,
                                animation.name.span.start.0,
                                animation.name.span.end.0,
                                animation.name.value.clone(),
                            ));
                            let identity = asset_symbol_name(
                                &animation.module_path,
                                AssetKind::Animation,
                                &animation.name,
                            );
                            let canonical = format!(
                                "{}\0dynamic-animation:{identity}:{}",
                                selector.value, options.value
                            );
                            let canonical = compiler_options.class_input(source.path(), &canonical);
                            let class_name = class_name(&canonical);
                            let mut utility = resolve_animation_use(
                                &animation,
                                &options,
                                theme,
                                &mut result.diagnostics,
                            );
                            utility.selector = Some(selector.value.clone());
                            if emitted_utility_classes
                                .insert((class_name.clone(), canonical.clone()))
                            {
                                emit_utility_rules(
                                    &mut utility_css,
                                    &class_name,
                                    &[utility],
                                    theme,
                                );
                                result.symbols.push(generated_class_symbol(
                                    source,
                                    &class_name,
                                    &canonical,
                                    call.span,
                                ));
                            }
                            classes.push((identity, class_name));
                        }
                        dynamic_asset_classes.push(DynamicAssetClassEntry { value, classes });
                        continue;
                    }
                    StyleInput::ContainerQuery {
                        handle,
                        query,
                        value,
                    } => {
                        let branch = &mut branches[0];
                        branch.2.push_str("container:");
                        branch.2.push_str(&ref_symbol_name(&handle));
                        branch.2.push(':');
                        branch.2.push_str(&query.value);
                        branch.2.push('\0');
                        for utility in lex_utilities(&value, &mut result.diagnostics) {
                            branch.2.push_str(&utility.value.raw);
                            branch.2.push('\0');
                            if let Some(mut utilities) =
                                resolve_utility_targets(&utility.value.raw, theme)
                            {
                                {
                                    for utility in &mut utilities {
                                        utility.container = Some(format!(
                                            "{} ({})",
                                            ref_symbol_name(&handle),
                                            query.value
                                        ));
                                    }
                                    branch.3.append(&mut utilities);
                                }
                            } else {
                                result.lint_findings.push(LintFinding::new(
                                    unresolved_utility_rule(&utility.value.raw),
                                    format!(
                                        "unknown or unavailable utility `{}`",
                                        utility.value.raw
                                    ),
                                    utility.span,
                                ));
                                external_classes.push(utility.span);
                            }
                        }
                        continue;
                    }
                    StyleInput::HandleProjection { handle, projection } => {
                        let branch = &mut branches[0];
                        let marker = handle_marker_class(&handle);
                        if !handle_markers.contains(&marker) {
                            handle_markers.push(marker);
                        }
                        branch.2.push_str("handle:");
                        branch.2.push_str(&ref_symbol_name(&handle));
                        branch.2.push(':');
                        branch.2.push_str(handle_projection_name(&projection));
                        branch.2.push('\0');
                        branch.3.push(CompiledUtility {
                            variants: Vec::new(),
                            declaration: render_handle_projection(&handle, &projection),
                            selector: None,
                            container: None,
                            nested: Vec::new(),
                            builtin_order: None,
                            candidate: String::new(),
                        });
                        continue;
                    }
                    StyleInput::DynamicHandleProjection {
                        value,
                        handles,
                        projection,
                    } => {
                        dynamic_handle_classes.push(value);
                        for handle in handles {
                            runtime_assets.insert(ref_identity(&handle));
                            let class_name = runtime_projection_class(&handle, &projection);
                            let canonical = format!(
                                "dynamic-handle:{}:{}",
                                ref_symbol_name(&handle),
                                handle_projection_name(&projection)
                            );
                            if emitted_utility_classes
                                .insert((class_name.clone(), canonical.clone()))
                            {
                                emit_utility_rules(
                                    &mut utility_css,
                                    &class_name,
                                    &[CompiledUtility {
                                        variants: Vec::new(),
                                        declaration: render_handle_projection(&handle, &projection),
                                        selector: None,
                                        container: None,
                                        nested: Vec::new(),
                                        builtin_order: None,
                                        candidate: String::new(),
                                    }],
                                    theme,
                                );
                                result.symbols.push(generated_class_symbol(
                                    source,
                                    &class_name,
                                    &canonical,
                                    call.span,
                                ));
                            }
                        }
                        continue;
                    }
                    StyleInput::ScopeAttachment(scope) => {
                        let marker = scope_marker_class(&scope)
                            .trim_start_matches('.')
                            .to_owned();
                        if !handle_markers.contains(&marker) {
                            handle_markers.push(marker);
                        }
                        branches[0].2.push_str("scope:");
                        branches[0].2.push_str(&scope.name.value);
                        branches[0].2.push('\0');
                        continue;
                    }
                    StyleInput::HandleAttachment(handle) => {
                        let marker = handle_marker_class(&handle);
                        if !handle_markers.contains(&marker) {
                            handle_markers.push(marker);
                        }
                        branches[0].2.push_str("ref:");
                        branches[0].2.push_str(&handle.name.value);
                        branches[0].2.push('\0');
                        continue;
                    }
                    StyleInput::DynamicScopeAttachment { value, scopes } => {
                        dynamic_handle_classes.push(value);
                        runtime_assets.extend(scopes.iter().map(scope_identity));
                        continue;
                    }
                    StyleInput::DynamicHandleAttachment { value, handles } => {
                        dynamic_handle_classes.push(value);
                        runtime_assets.extend(handles.iter().map(ref_identity));
                        continue;
                    }
                    StyleInput::ExternalClass(value) => {
                        external_classes.push(value);
                        continue;
                    }
                    StyleInput::UtilityString(input) => (None, true, None, input),
                    StyleInput::ConditionalUtilityString {
                        guard,
                        when_true,
                        value,
                    } => (Some(guard), when_true, None, value),
                    StyleInput::Attributes(entries) => {
                        attributes.extend(entries);
                        continue;
                    }
                    StyleInput::SelectorUtilityString { selector, value } => {
                        (None, true, Some(selector.value), value)
                    }
                    StyleInput::ConditionalSelectorUtilityString {
                        selector,
                        guard,
                        when_true,
                        value,
                    } => (Some(guard), when_true, Some(selector.value), value),
                    StyleInput::HandleSelectorUtilityString {
                        handle,
                        selector,
                        value,
                    } => {
                        let Some(parent) = handle_parent_selector(&handle, &selector.value) else {
                            result.diagnostics.push(CompileError::new(
                                "SHREF003",
                                "ref selectors must begin with `&`",
                                selector.span,
                            ));
                            continue;
                        };
                        (None, true, Some(parent), value)
                    }
                    StyleInput::ConditionalHandleSelectorUtilityString {
                        handle,
                        selector,
                        guard,
                        when_true,
                        value,
                    } => {
                        let Some(parent) = handle_parent_selector(&handle, &selector.value) else {
                            result.diagnostics.push(CompileError::new(
                                "SHREF003",
                                "ref selectors must begin with `&`",
                                selector.span,
                            ));
                            continue;
                        };
                        (Some(guard), when_true, Some(parent), value)
                    }
                };
                let branch_index = if let Some(guard) = guard {
                    branches.push((Some(guard), guard_when_true, String::new(), Vec::new()));
                    branches.len() - 1
                } else {
                    0
                };
                let branch = &mut branches[branch_index];
                for utility in lex_utilities(&input, &mut result.diagnostics) {
                    if (options.forbid_tailwind_relationships
                        || theme.config.lint_enabled("prefer-typed-relationships"))
                        && is_tailwind_relationship_candidate(&utility.value.raw, theme)
                    {
                        result.lint_findings.push(LintFinding::new(
                            "prefer-typed-relationships",
                            format!(
                                "Tailwind relationship utility `{}` is forbidden; use style.ref() or style.scope()",
                                utility.value.raw
                            ),
                            utility.span,
                        ));
                    }
                    if selector.is_none()
                        && let Some(marker) = compatibility_marker_class(&utility.value.raw, theme)
                    {
                        let class = GeneratedClass {
                            class_name: marker,
                            call_span: call.span,
                            guard,
                            guard_when_true,
                        };
                        if !compatibility_classes.contains(&class) {
                            compatibility_classes.push(class);
                        }
                        continue;
                    }
                    if let Some(selector) = &selector {
                        branch.2.push_str(selector);
                        branch.2.push('\0');
                    }
                    branch.2.push_str(&utility.value.raw);
                    branch.2.push('\0');
                    let resolved_input = utility
                        .value
                        .raw
                        .strip_prefix("tw:")
                        .unwrap_or(&utility.value.raw);
                    if let Some(mut utilities) = resolve_utility_targets(resolved_input, theme) {
                        let candidate_variants = parse_candidate(resolved_input)
                            .map(|candidate| candidate.variants)
                            .unwrap_or_default();
                        {
                            for resolved_utility in &mut utilities {
                                resolved_utility.variants.clone_from(&candidate_variants);
                                if let Some(selector) = &selector {
                                    resolved_utility.selector =
                                        Some(resolved_utility.selector.as_deref().map_or_else(
                                            || selector.clone(),
                                            |nested| nested.replace('&', selector),
                                        ));
                                }
                            }
                            branch.3.append(&mut utilities);
                        }
                    } else {
                        result.lint_findings.push(LintFinding::new(
                            unresolved_utility_rule(&utility.value.raw),
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        ));
                        if selector.is_none() {
                            external_classes.push(utility.span);
                        }
                    }
                }
            }

            let mut call_classes = Vec::new();
            for (guard, guard_when_true, canonical_input, utilities) in branches {
                if utilities.is_empty() {
                    continue;
                }
                let canonical_input = options.class_input(source.path(), &canonical_input);
                let class_name = class_name(&canonical_input);
                if emitted_utility_classes.insert((class_name.clone(), canonical_input.clone())) {
                    emit_utility_rules(&mut utility_css, &class_name, &utilities, theme);
                    result.symbols.push(generated_class_symbol(
                        source,
                        &class_name,
                        &canonical_input,
                        call.span,
                    ));
                }
                let generated_class = GeneratedClass {
                    class_name,
                    call_span: call.span,
                    guard,
                    guard_when_true,
                };
                call_classes.push(generated_class.clone());
                result.classes.push(generated_class);
            }
            for class in compatibility_classes {
                call_classes.push(class.clone());
                result.classes.push(class);
            }
            if let Some(replacement) = render_react_props(
                source,
                &ReactPropsParts {
                    classes: &call_classes,
                    attributes: &attributes,
                    handle_markers: &handle_markers,
                    external_classes: &external_classes,
                    dynamic_handle_classes: &dynamic_handle_classes,
                    dynamic_asset_classes: &dynamic_asset_classes,
                    view_transition_style: view_transition_style.as_ref(),
                },
                module.adapter,
            ) {
                result.transforms.push(SourceTransform {
                    call_span: call.span,
                    replacement,
                });
            }
        }
        for handle_definition in module.handle_definitions {
            result.transforms.push(SourceTransform {
                call_span: handle_definition.call_span,
                replacement: if runtime_assets.contains(&ref_identity(&handle_definition)) {
                    format!(
                        "stillwindRef(\"{}\")",
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
                        "stillwindScope(\"{}\")",
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
                        "stillwindAnimation(\"{}\")",
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
                        "stillwindViewTransition(\"{}\")",
                        symbol_name(source, AssetKind::ViewTransition, &transition.name)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }

        if !result.diagnostics.is_empty() {
            result.css.clear();
            result.classes.clear();
            result.symbols.clear();
            result.transforms.clear();
        } else if !utility_css.is_empty()
            || !theme.custom_css.is_empty()
            || !theme.output_template.nodes.is_empty()
        {
            let utility_css = options.rewrite_generated(&utility_css);
            result.module_css.clone_from(&utility_css);
            match render_stylesheet(theme, &result.module_css, options) {
                Ok(css) => result.css = css,
                Err(error) => {
                    result.diagnostics.push(CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        source.full_span(),
                    ));
                    result.module_css.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
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
