use super::{
    AssetKind, BTreeSet, CompileError, CompileResult, CompiledUtility, Compiler,
    DesignSystemSnapshot, DynamicAssetClassEntry, GeneratedClass, IdentityId, LintFinding,
    PropsTransform, ReactPropsParts, SourceFile, StyleInput, ViewTransitionStyleEntry,
    asset_symbol_name, class_name, emit_animation_definition, emit_utility_rules,
    emit_view_transition_definition, filter_lint_findings, generated_class_symbol,
    generated_ref_marker_symbol, generated_ref_symbol, generated_scope_marker_symbol,
    generated_scope_symbol, generated_symbol, handle_marker_class, handle_parent_selector,
    handle_projection_name, lex_utilities, named_identity, parse_candidate, ref_identity,
    ref_symbol_name, render_handle_projection, render_react_props, render_stylesheet,
    resolve_animation_use, resolve_utility_targets, runtime_projection_class, scope_identity,
    scope_marker_class, summary_runtime_assets, symbol_name, unresolved_utility_rule,
    view_transition_utility,
};

impl Compiler {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_with_snapshot(
        source: &SourceFile,
        parsed: stillwind_js_oxc::ParsedJavaScript,
        snapshot: &DesignSystemSnapshot,
    ) -> CompileResult {
        let mut runtime_assets = summary_runtime_assets(&parsed.summary);
        let mut result = CompileResult {
            diagnostics: parsed.diagnostics,
            lint_findings: parsed.lint_findings,
            ..CompileResult::default()
        };
        let theme = snapshot.theme();
        let options = snapshot.config();
        result.lint_policy.clone_from(&theme.config.lint_rules);
        filter_lint_findings(source, &mut result.lint_findings);

        if !result.diagnostics.is_empty() {
            return result;
        }

        let mut utility_css = String::new();
        let mut emitted_utility_classes = BTreeSet::new();
        let module = parsed.module;
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
                .map(|definition| generated_symbol(source, "animation", &definition.name)),
        );
        result.symbols.extend(
            module
                .view_transition_declarations
                .iter()
                .map(|definition| generated_symbol(source, "view-transition", &definition.name)),
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
                if let StyleInput::ViewTransitionUse { transition, key } = input {
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
                        "view-transition",
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
                if let StyleInput::DynamicViewTransitionUse {
                    value,
                    transitions,
                    key,
                } = input
                {
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
                            "view-transition",
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
                if let StyleInput::AnimationUse { animation, options } = input {
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
                if let StyleInput::DynamicAnimationUse {
                    value,
                    animations,
                    options,
                } = input
                {
                    let mut classes = Vec::new();
                    for animation in animations {
                        runtime_assets.insert(IdentityId::new(
                            AssetKind::Animation,
                            &animation.module_path,
                            animation.name.span.start.0,
                            animation.name.span.end.0,
                            animation.name.value.clone(),
                        ));
                        let identity =
                            asset_symbol_name(&animation.module_path, "animation", &animation.name);
                        let canonical = format!("dynamic-animation:{identity}:{}", options.value);
                        let class_name = class_name(&canonical);
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
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
                if let StyleInput::SelectorAnimationUse {
                    selector,
                    animation,
                    options,
                } = input
                {
                    let branch = &mut branches[0];
                    branch.2.push_str(&selector.value);
                    branch.2.push('\0');
                    branch.2.push_str("animation:");
                    branch.2.push_str(&animation.name.value);
                    branch.2.push(':');
                    branch.2.push_str(&options.value);
                    branch.2.push('\0');
                    let mut utility =
                        resolve_animation_use(&animation, &options, theme, &mut result.diagnostics);
                    utility.selector = Some(selector.value);
                    branch.3.push(utility);
                    continue;
                }
                if let StyleInput::DynamicSelectorAnimationUse {
                    selector,
                    value,
                    animations,
                    options,
                } = input
                {
                    let mut classes = Vec::new();
                    for animation in animations {
                        runtime_assets.insert(IdentityId::new(
                            AssetKind::Animation,
                            &animation.module_path,
                            animation.name.span.start.0,
                            animation.name.span.end.0,
                            animation.name.value.clone(),
                        ));
                        let identity =
                            asset_symbol_name(&animation.module_path, "animation", &animation.name);
                        let canonical = format!(
                            "{}\0dynamic-animation:{identity}:{}",
                            selector.value, options.value
                        );
                        let class_name = class_name(&canonical);
                        let mut utility = resolve_animation_use(
                            &animation,
                            &options,
                            theme,
                            &mut result.diagnostics,
                        );
                        utility.selector = Some(selector.value.clone());
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
                            emit_utility_rules(&mut utility_css, &class_name, &[utility], theme);
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
                if let StyleInput::ContainerQuery {
                    handle,
                    query,
                    value,
                } = input
                {
                    let branch = &mut branches[0];
                    branch.2.push_str("container:");
                    branch.2.push_str(&ref_symbol_name(&handle));
                    branch.2.push(':');
                    branch.2.push_str(&query.value);
                    branch.2.push('\0');
                    for utility in lex_utilities(&value) {
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
                                format!("unknown or unavailable utility `{}`", utility.value.raw),
                                utility.span,
                            ));
                            external_classes.push(utility.span);
                        }
                    }
                    continue;
                }
                if let StyleInput::HandleProjection { handle, projection } = input {
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
                if let StyleInput::DynamicHandleProjection {
                    value,
                    handles,
                    projection,
                } = input
                {
                    dynamic_handle_classes.push(value);
                    for handle in handles {
                        runtime_assets.insert(ref_identity(&handle));
                        let class_name = runtime_projection_class(&handle, &projection);
                        let canonical = format!(
                            "dynamic-handle:{}:{}",
                            ref_symbol_name(&handle),
                            handle_projection_name(&projection)
                        );
                        if emitted_utility_classes.insert((class_name.clone(), canonical.clone())) {
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
                if let StyleInput::ScopeAttachment(scope) = input {
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
                if let StyleInput::HandleAttachment(handle) = input {
                    let marker = handle_marker_class(&handle);
                    if !handle_markers.contains(&marker) {
                        handle_markers.push(marker);
                    }
                    branches[0].2.push_str("ref:");
                    branches[0].2.push_str(&handle.name.value);
                    branches[0].2.push('\0');
                    continue;
                }
                if let StyleInput::DynamicScopeAttachment { value, scopes } = input {
                    dynamic_handle_classes.push(value);
                    runtime_assets.extend(scopes.iter().map(scope_identity));
                    continue;
                }
                if let StyleInput::DynamicHandleAttachment { value, handles } = input {
                    dynamic_handle_classes.push(value);
                    runtime_assets.extend(handles.iter().map(ref_identity));
                    continue;
                }
                if let StyleInput::ExternalClass(value) = input {
                    external_classes.push(value);
                    continue;
                }
                let (guard, guard_when_true, selector, input) = match input {
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
                    } => (
                        None,
                        true,
                        Some(handle_parent_selector(&handle, &selector.value)),
                        value,
                    ),
                    StyleInput::ConditionalHandleSelectorUtilityString {
                        handle,
                        selector,
                        guard,
                        when_true,
                        value,
                    } => (
                        Some(guard),
                        when_true,
                        Some(handle_parent_selector(&handle, &selector.value)),
                        value,
                    ),
                    StyleInput::HandleProjection { .. }
                    | StyleInput::DynamicHandleProjection { .. }
                    | StyleInput::ScopeAttachment(_)
                    | StyleInput::HandleAttachment(_)
                    | StyleInput::DynamicScopeAttachment { .. }
                    | StyleInput::DynamicHandleAttachment { .. }
                    | StyleInput::ExternalClass(_)
                    | StyleInput::ContainerQuery { .. }
                    | StyleInput::AnimationUse { .. }
                    | StyleInput::DynamicAnimationUse { .. }
                    | StyleInput::SelectorAnimationUse { .. }
                    | StyleInput::DynamicSelectorAnimationUse { .. }
                    | StyleInput::ViewTransitionUse { .. }
                    | StyleInput::DynamicViewTransitionUse { .. } => {
                        unreachable!("handled above")
                    }
                };
                let branch_index = if let Some(guard) = guard {
                    branches.push((Some(guard), guard_when_true, String::new(), Vec::new()));
                    branches.len() - 1
                } else {
                    0
                };
                let branch = &mut branches[branch_index];
                for utility in lex_utilities(&input) {
                    if (options.forbid_tailwind_relationships
                        || theme.config.lint_enabled("prefer-typed-relationships"))
                        && stillwind_tailwind::is_tailwind_relationship_candidate(
                            &utility.value.raw,
                            theme,
                        )
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
                        && let Some(marker) = stillwind_tailwind::compatibility_marker_class(
                            &utility.value.raw,
                            theme,
                        )
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
            ) {
                result.transforms.push(PropsTransform {
                    call_span: call.span,
                    replacement,
                });
            }
        }
        for handle_definition in module.handle_definitions {
            result.transforms.push(PropsTransform {
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
            result.transforms.push(PropsTransform {
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
            result.transforms.push(PropsTransform {
                call_span: animation.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::Animation,
                    &animation.name,
                )) {
                    format!(
                        "stillwindAnimation(\"{}\")",
                        symbol_name(source, "animation", &animation.name)
                    )
                } else {
                    "undefined".to_owned()
                },
            });
        }
        for transition in module.view_transition_declarations {
            result.transforms.push(PropsTransform {
                call_span: transition.call_span,
                replacement: if runtime_assets.contains(&named_identity(
                    source,
                    AssetKind::ViewTransition,
                    &transition.name,
                )) {
                    format!(
                        "stillwindViewTransition(\"{}\")",
                        symbol_name(source, "view-transition", &transition.name)
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
            || theme.imports.theme_layer.is_some()
            || theme.imports.base_layer.is_some()
            || theme.imports.utilities_layer.is_some()
        {
            let utility_css = options.rewrite_generated(&utility_css);
            result.module_css.clone_from(&utility_css);
            result.css = render_stylesheet(theme, &result.module_css, options);
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
