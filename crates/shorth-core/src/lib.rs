//! Public orchestration API for the first compiler slice.

use std::fmt::Write;

use serde::Serialize;
use shorth_grammar::{Variant, lex_utilities};
use shorth_ir::{
    AnimationDefinition, AttributeEntry, HandleProjection, StyleInput, UnsafeDeclaration,
    ViewTransitionDefinition,
};
use shorth_source::{Diagnostic, SourceFile, Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

/// A generated CSS identifier with the canonical identity used to allocate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug)]
struct DynamicStyleEntry {
    property: String,
    key: Span,
    prefix: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, String> {
        let mut transforms = self.transforms.clone();
        transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

        let mut output = source.text().to_owned();
        let mut previous_start = u32::MAX;
        for transform in transforms {
            if transform.call_span.file != source.id()
                || transform.call_span.end.0 > previous_start
                || transform.call_span.end.0 as usize > output.len()
            {
                return Err("invalid or overlapping shorth transform plan".to_owned());
            }
            let start = transform.call_span.start.0 as usize;
            let end = transform.call_span.end.0 as usize;
            output.replace_range(start..end, &transform.replacement);
            previous_start = transform.call_span.start.0;
        }
        Ok(output)
    }
}

pub struct Compiler;

impl Compiler {
    pub fn check_javascript(source: &SourceFile) -> Vec<Diagnostic> {
        shorth_js_oxc::parse(source).diagnostics
    }

    pub fn analyze_javascript(source: &SourceFile) -> shorth_js_oxc::ParsedJavaScript {
        shorth_js_oxc::parse(source)
    }

    /// Reads the project's CSS-first theme surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the stylesheet is invalid or contains an
    /// unterminated `@theme` block.
    pub fn read_theme(
        source: &SourceFile,
    ) -> Result<shorth_css::ThemeIndex, shorth_css::ThemeError> {
        shorth_css::parse_tailwind_theme(source)
    }

    /// Generates a module augmentation containing the theme-backed utility vocabulary.
    ///
    /// # Errors
    ///
    /// Returns an error when the CSS theme cannot be parsed.
    pub fn generate_react_types(
        theme_source: &SourceFile,
    ) -> Result<String, shorth_css::ThemeError> {
        let theme = Self::read_theme(theme_source)?;
        Ok(render_react_types(&theme))
    }

    /// Compiles the first static React subset into CSS.
    ///
    /// This temporary API deliberately supports only literal utility-string
    /// arguments. Conditions, attributes, selectors, and transforms are added
    /// in successive compiler slices.
    #[must_use]
    #[allow(clippy::too_many_lines)] // Coordinates source transforms with CSS emission.
    pub fn compile_react(source: &SourceFile, theme_source: &SourceFile) -> CompileResult {
        let parsed = Self::analyze_javascript(source);
        let mut result = CompileResult {
            diagnostics: parsed.diagnostics,
            ..CompileResult::default()
        };
        let theme = match Self::read_theme(theme_source) {
            Ok(theme) => theme,
            Err(error) => {
                result.diagnostics.push(Diagnostic::error(
                    "SHCSS001",
                    error.to_string(),
                    theme_source.full_span(),
                ));
                return result;
            }
        };

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == shorth_source::Severity::Error)
        {
            return result;
        }

        let mut utility_css = String::new();
        let module = parsed.module;
        result.symbols.extend(
            module
                .handle_definitions
                .iter()
                .map(|definition| generated_symbol(source, "handle", &definition.name)),
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
                &theme,
                &mut result.diagnostics,
            );
        }
        for transition in &module.view_transition_declarations {
            emit_view_transition_definition(
                &mut utility_css,
                source,
                transition,
                &theme,
                &mut result.diagnostics,
            );
        }
        for call in module.style_calls {
            let mut branches = vec![(None, true, String::new(), Vec::new())];
            let mut attributes = Vec::new();
            let mut handle_markers = Vec::new();
            let mut dynamic_styles = Vec::new();
            for input in call.inputs {
                if let StyleInput::Unsafe {
                    selector,
                    declarations,
                    reason: _,
                } = input
                {
                    let branch = &mut branches[0];
                    branch.2.push_str("unsafe:");
                    branch.2.push_str(&selector.value);
                    branch.2.push('\0');
                    if let Some(declaration) = unsafe_declaration_list(source, &declarations) {
                        branch.2.push_str(&declaration);
                        branch.2.push('\0');
                        branch.3.push(CompiledUtility {
                            variants: Vec::new(),
                            declaration,
                            selector: Some(selector.value),
                            container: None,
                        });
                    }
                    continue;
                }
                if let StyleInput::ViewTransitionUse { transition, key } = input {
                    let name = symbol_name(source, "view-transition", &transition);
                    let branch = &mut branches[0];
                    branch.2.push_str("view-transition:");
                    branch.2.push_str(&transition.value);
                    if let Some(key) = key {
                        branch.2.push_str(":keyed");
                        dynamic_styles.push(DynamicStyleEntry {
                            property: name.clone(),
                            key,
                            prefix: format!("{name}-"),
                        });
                        branch.3.push(CompiledUtility {
                            variants: Vec::new(),
                            declaration: format!("view-transition-name:var({name});"),
                            selector: None,
                            container: None,
                        });
                    } else {
                        branch.3.push(CompiledUtility {
                            variants: Vec::new(),
                            declaration: format!("view-transition-name:{name};"),
                            selector: None,
                            container: None,
                        });
                    }
                    branch.2.push('\0');
                    continue;
                }
                if let StyleInput::AnimationUse { animation, options } = input {
                    let branch = &mut branches[0];
                    branch.2.push_str("animation:");
                    branch.2.push_str(&animation.value);
                    branch.2.push(':');
                    branch.2.push_str(&options.value);
                    branch.2.push('\0');
                    branch.3.push(resolve_animation_use(
                        source,
                        &animation,
                        &options,
                        &theme,
                        &mut result.diagnostics,
                    ));
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
                    branch.2.push_str(&animation.value);
                    branch.2.push(':');
                    branch.2.push_str(&options.value);
                    branch.2.push('\0');
                    let mut utility = resolve_animation_use(
                        source,
                        &animation,
                        &options,
                        &theme,
                        &mut result.diagnostics,
                    );
                    utility.selector = Some(selector.value);
                    branch.3.push(utility);
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
                    branch.2.push_str(&handle.value);
                    branch.2.push(':');
                    branch.2.push_str(&query.value);
                    branch.2.push('\0');
                    for utility in lex_utilities(&value) {
                        branch.2.push_str(&utility.value.raw);
                        branch.2.push('\0');
                        match resolve_utility(&utility.value.raw, &theme) {
                            Some(mut utility) => {
                                utility.container = Some(format!(
                                    "{} ({})",
                                    symbol_name(source, "handle", &handle),
                                    query.value
                                ));
                                branch.3.push(utility);
                            }
                            None => result.diagnostics.push(Diagnostic::error(
                                "SH001",
                                format!("unknown or unavailable utility `{}`", utility.value.raw),
                                utility.span,
                            )),
                        }
                    }
                    continue;
                }
                if let StyleInput::HandleProjection { handle, projection } = input {
                    let branch = &mut branches[0];
                    let marker = handle_marker_class(source, &handle);
                    if !handle_markers.contains(&marker) {
                        handle_markers.push(marker);
                    }
                    branch.2.push_str("handle:");
                    branch.2.push_str(&handle.value);
                    branch.2.push(':');
                    branch.2.push_str(handle_projection_name(&projection));
                    branch.2.push('\0');
                    branch.3.push(CompiledUtility {
                        variants: Vec::new(),
                        declaration: render_handle_projection(source, &handle, &projection),
                        selector: None,
                        container: None,
                    });
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
                        Some(handle_parent_selector(source, &handle, &selector.value)),
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
                        Some(handle_parent_selector(source, &handle, &selector.value)),
                        value,
                    ),
                    StyleInput::HandleProjection { .. }
                    | StyleInput::ContainerQuery { .. }
                    | StyleInput::AnimationUse { .. }
                    | StyleInput::SelectorAnimationUse { .. }
                    | StyleInput::ViewTransitionUse { .. }
                    | StyleInput::Unsafe { .. } => {
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
                    if let Some(selector) = &selector {
                        branch.2.push_str(selector);
                        branch.2.push('\0');
                    }
                    branch.2.push_str(&utility.value.raw);
                    branch.2.push('\0');
                    match resolve_utility(&utility.value.raw, &theme) {
                        Some(mut utility) => {
                            if let Some(selector) = &selector {
                                utility.selector = Some(utility.selector.as_deref().map_or_else(
                                    || selector.clone(),
                                    |nested| nested.replace('&', selector),
                                ));
                            }
                            branch.3.push(utility);
                        }
                        None => result.diagnostics.push(Diagnostic::error(
                            "SH001",
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        )),
                    }
                }
            }

            let mut call_classes = Vec::new();
            for (guard, guard_when_true, canonical_input, utilities) in branches {
                if utilities.is_empty() {
                    continue;
                }
                let class_name = class_name(&canonical_input);
                emit_utility_rules(&mut utility_css, &class_name, &utilities, &theme);
                result.symbols.push(generated_class_symbol(
                    source,
                    &class_name,
                    &canonical_input,
                    call.span,
                ));
                let generated_class = GeneratedClass {
                    class_name,
                    call_span: call.span,
                    guard,
                    guard_when_true,
                };
                call_classes.push(generated_class.clone());
                result.classes.push(generated_class);
            }
            if let Some(replacement) = render_react_props(
                source,
                &call_classes,
                &attributes,
                &handle_markers,
                &dynamic_styles,
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
                replacement: "undefined".to_owned(),
            });
        }
        for animation in module.animation_declarations {
            result.transforms.push(PropsTransform {
                call_span: animation.call_span,
                replacement: "undefined".to_owned(),
            });
        }
        for transition in module.view_transition_declarations {
            result.transforms.push(PropsTransform {
                call_span: transition.call_span,
                replacement: "undefined".to_owned(),
            });
        }

        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == shorth_source::Severity::Error)
        {
            result.css.clear();
            result.classes.clear();
            result.symbols.clear();
            result.transforms.clear();
        } else if !result.classes.is_empty() {
            result.css = render_stylesheet(&theme, &utility_css);
        }

        result
    }
}

fn handle_projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
    }
}

fn normalized_module_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn symbol_identity(source: &SourceFile, kind: &str, definition: &Spanned<String>) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        normalized_module_path(source.path()),
        definition.span.start.0,
        definition.span.end.0,
        definition.value,
    )
}

fn symbol_name(source: &SourceFile, kind: &str, definition: &Spanned<String>) -> String {
    let prefix = match kind {
        "handle" => "--sw-h-",
        "animation" => "--sw-a-",
        "view-transition" => "--sw-v-",
        _ => unreachable!("only named Stillwind asset kinds are allocated"),
    };
    let identity = symbol_identity(source, kind, definition);
    let hash = blake3::hash(identity.as_bytes());
    format!("{prefix}{}", &hash.to_hex()[..8])
}

fn generated_symbol(
    source: &SourceFile,
    kind: &str,
    definition: &Spanned<String>,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: symbol_name(source, kind, definition),
        kind: kind.to_owned(),
        identity: symbol_identity(source, kind, definition),
        source_path: normalized_module_path(source.path()),
        start: definition.span.start.0,
        end: definition.span.end.0,
    }
}

fn generated_class_symbol(
    source: &SourceFile,
    name: &str,
    canonical_input: &str,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: "class".to_owned(),
        identity: format!("stillwind/v1\0class\0{canonical_input}"),
        source_path: normalized_module_path(source.path()),
        start: span.start.0,
        end: span.end.0,
    }
}

fn emit_animation_definition(
    output: &mut String,
    source: &SourceFile,
    animation: &AnimationDefinition,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    output.push_str("@keyframes ");
    output.push_str(&symbol_name(source, "animation", &animation.name));
    output.push('{');
    for frame in &animation.frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities) {
            match resolve_utility(&utility.value.raw, theme) {
                Some(utility) if utility.variants.is_empty() => utilities.push(utility),
                Some(_) => diagnostics.push(Diagnostic::error(
                    "SHANIMATION003",
                    "animation keyframes cannot contain variants",
                    utility.span,
                )),
                None => diagnostics.push(Diagnostic::error(
                    "SH001",
                    format!("unknown or unavailable utility `{}`", utility.value.raw),
                    utility.span,
                )),
            }
        }
        output.push_str(&frame.selector.value);
        output.push('{');
        output.push_str(&combine_declarations(&utilities.iter().collect::<Vec<_>>()));
        output.push('}');
    }
    output.push('}');
}

fn resolve_animation_use(
    source: &SourceFile,
    animation: &Spanned<String>,
    options: &Spanned<String>,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        symbol_name(source, "animation", animation)
    );
    declaration.push_str(&resolve_motion_options(options, theme, diagnostics));
    CompiledUtility {
        variants: Vec::new(),
        declaration,
        selector: None,
        container: None,
    }
}

fn resolve_motion_options(
    options: &Spanned<String>,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut declaration = String::new();
    for option in lex_utilities(options) {
        match option.value.raw.as_str() {
            "none" | "forwards" | "backwards" | "both" => {
                write!(
                    &mut declaration,
                    "animation-fill-mode:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "running" | "paused" => {
                write!(
                    &mut declaration,
                    "animation-play-state:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "normal" | "reverse" | "alternate" | "alternate-reverse" => {
                write!(
                    &mut declaration,
                    "animation-direction:{};",
                    option.value.raw
                )
                .expect("writing to a String cannot fail");
            }
            "infinite" => declaration.push_str("animation-iteration-count:infinite;"),
            value => match resolve_utility(value, theme) {
                Some(utility)
                    if utility.variants.is_empty()
                        && (utility.declaration.starts_with("transition-duration:")
                            || utility
                                .declaration
                                .starts_with("transition-timing-function:")) =>
                {
                    declaration.push_str(&utility.declaration.replace("transition-", "animation-"));
                }
                _ => diagnostics.push(Diagnostic::error(
                    "SHANIMATION002",
                    format!("unknown animation option `{value}`"),
                    option.span,
                )),
            },
        }
    }
    declaration
}

fn emit_view_transition_definition(
    output: &mut String,
    source: &SourceFile,
    transition: &ViewTransitionDefinition,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = symbol_name(source, "view-transition", &transition.name);
    if let Some(group) = &transition.group {
        output.push_str("::view-transition-group(");
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_motion_options(group, theme, diagnostics));
        output.push('}');
    }
    for (pseudo, utilities) in [
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        output.push_str("::view-transition-");
        output.push_str(pseudo);
        output.push('(');
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_static_utility_list(utilities, theme, diagnostics));
        output.push('}');
    }
}

fn resolve_static_utility_list(
    input: &Spanned<String>,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input) {
        match resolve_utility(&utility.value.raw, theme) {
            Some(utility) if utility.variants.is_empty() => utilities.push(utility),
            Some(_) => diagnostics.push(Diagnostic::error(
                "SHMOTION001",
                "motion definition utilities cannot contain variants",
                utility.span,
            )),
            None => diagnostics.push(Diagnostic::error(
                "SH001",
                format!("unknown or unavailable utility `{}`", utility.value.raw),
                utility.span,
            )),
        }
    }
    combine_declarations(&utilities.iter().collect::<Vec<_>>())
}

fn unsafe_declaration_list(
    source: &SourceFile,
    declarations: &[UnsafeDeclaration],
) -> Option<String> {
    let mut output = String::new();
    for declaration in declarations {
        output.push_str(&camel_to_kebab(&declaration.property.value));
        output.push(':');
        output.push_str(source.slice(declaration.value).ok()?);
        output.push(';');
    }
    Some(output)
}

fn camel_to_kebab(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            output.push('-');
            output.extend(character.to_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

fn render_handle_projection(
    source: &SourceFile,
    handle: &Spanned<String>,
    projection: &HandleProjection,
) -> String {
    let name = symbol_name(source, "handle", handle);
    match projection {
        HandleProjection::Anchor => format!("anchor-name:{name};"),
        HandleProjection::Position => format!("position-anchor:{name};"),
        HandleProjection::Scope => format!("anchor-scope:{name};"),
        HandleProjection::Container(kind) => {
            format!("container-name:{name};container-type:{};", kind.value)
        }
    }
}

fn handle_marker_class(source: &SourceFile, handle: &Spanned<String>) -> String {
    symbol_name(source, "handle", handle)
        .trim_start_matches("--")
        .to_owned()
}

fn handle_parent_selector(source: &SourceFile, handle: &Spanned<String>, selector: &str) -> String {
    let marker = handle_marker_class(source, handle);
    format!("{} &", selector.replace('&', &format!(".{marker}")))
}

fn render_react_props(
    source: &SourceFile,
    classes: &[GeneratedClass],
    attributes: &[AttributeEntry],
    handle_markers: &[String],
    dynamic_styles: &[DynamicStyleEntry],
) -> Option<String> {
    let mut entries = Vec::new();
    if !classes.is_empty() || !handle_markers.is_empty() {
        let mut class_names = handle_markers
            .iter()
            .map(|marker| format!("\"{marker}\""))
            .collect::<Vec<_>>();
        class_names.extend(
            classes
                .iter()
                .map(|class| match class.guard {
                    Some(guard) => source.slice(guard).ok().map(|guard| {
                        let guard = if class.guard_when_true {
                            guard.to_owned()
                        } else {
                            format!("!{guard}")
                        };
                        format!("{guard}&&\"{}\"", class.class_name)
                    }),
                    None => Some(format!("\"{}\"", class.class_name)),
                })
                .collect::<Option<Vec<_>>>()?,
        );
        let class_value = if class_names.len() == 1 {
            class_names.into_iter().next()?
        } else {
            format!("stillwindClsx({})", class_names.join(","))
        };
        entries.push(format!("className:{class_value}"));
    }
    for attribute in attributes {
        let value = source.slice(attribute.value).ok()?;
        entries.push(format!("\"{}\":{value}", attribute.name.value));
    }
    if !dynamic_styles.is_empty() {
        let styles = dynamic_styles
            .iter()
            .map(|entry| {
                source
                    .slice(entry.key)
                    .ok()
                    .map(|key| format!("\"{}\":`{}${{{key}}}`", entry.property, entry.prefix))
            })
            .collect::<Option<Vec<_>>>()?;
        entries.push(format!("style:{{{}}}", styles.join(",")));
    }
    Some(format!("{{{}}}", entries.join(",")))
}

#[derive(Clone, Debug)]
struct CompiledUtility {
    variants: Vec<Variant>,
    declaration: String,
    selector: Option<String>,
    container: Option<String>,
}

fn resolve_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<CompiledUtility> {
    let resolved = shorth_tailwind::resolve_utility(utility, theme)?;
    Some(CompiledUtility {
        variants: resolved.candidate.variants,
        declaration: resolved.declaration,
        selector: resolved.selector,
        container: None,
    })
}

fn emit_utility_rules(
    output: &mut String,
    class_name: &str,
    utilities: &[CompiledUtility],
    theme: &shorth_css::ThemeIndex,
) {
    let base = utilities
        .iter()
        .filter(|utility| {
            utility.variants.is_empty() && utility.selector.is_none() && utility.container.is_none()
        })
        .collect::<Vec<_>>();
    let base = combine_declarations(&base);
    if !base.is_empty() {
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        output.push_str(&base);
        output.push('}');
    }
    for utility in utilities.iter().filter(|utility| {
        !utility.variants.is_empty() || utility.selector.is_some() || utility.container.is_some()
    }) {
        let class_selector = format!(".{class_name}");
        let mut selector = utility.selector.as_deref().map_or_else(
            || class_selector.clone(),
            |selector| selector.replace('&', &class_selector),
        );
        let mut media = Vec::new();
        let mut containers = Vec::new();
        let mut supports = Vec::new();
        let mut starting_style = false;
        for variant in &utility.variants {
            match shorth_tailwind::resolve_variant(variant, theme) {
                Some(shorth_tailwind::VariantApplication::SelectorSuffix(suffix)) => {
                    selector.push_str(suffix);
                }
                Some(shorth_tailwind::VariantApplication::SelectorTemplate(template)) => {
                    selector = template.replace('&', &selector);
                }
                Some(shorth_tailwind::VariantApplication::SelectorMedia {
                    selector: template,
                    condition,
                }) => {
                    selector = template.replace('&', &selector);
                    media.push(condition);
                }
                Some(shorth_tailwind::VariantApplication::Media(condition)) => {
                    media.push(condition);
                }
                Some(shorth_tailwind::VariantApplication::Container(condition)) => {
                    containers.push(condition);
                }
                Some(shorth_tailwind::VariantApplication::Supports(condition)) => {
                    supports.push(condition);
                }
                Some(shorth_tailwind::VariantApplication::StartingStyle) => {
                    starting_style = true;
                }
                None => {}
            }
        }
        let mut rule = format!("{selector}{{{}}}", combine_declarations(&[utility]));
        for condition in supports.into_iter().rev() {
            rule = format!("@supports ({condition}){{{rule}}}");
        }
        for condition in media.into_iter().rev() {
            rule = format!("@media {condition}{{{rule}}}");
        }
        for condition in containers.into_iter().rev() {
            rule = format!("@container {condition}{{{rule}}}");
        }
        if starting_style {
            rule = format!("@starting-style{{{rule}}}");
        }
        if let Some(container) = &utility.container {
            rule = format!("@container {container}{{{rule}}}");
        }
        output.push_str(&rule);
    }
}

fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    let mut declarations = String::new();
    let mut transforms = Vec::new();
    for utility in utilities {
        for declaration in utility.declaration.split_terminator(';') {
            if let Some(transform) = declaration.strip_prefix("transform:") {
                transforms.push(transform);
            } else if !declaration.is_empty() {
                declarations.push_str(declaration);
                declarations.push(';');
            }
        }
    }
    if !transforms.is_empty() {
        declarations.push_str("transform:");
        declarations.push_str(&transforms.join(" "));
        declarations.push(';');
    }
    declarations
}

fn render_stylesheet(theme: &shorth_css::ThemeIndex, utility_css: &str) -> String {
    let mut css =
        String::from("@layer theme, base, components, utilities;\n@layer theme{:root,:host{");
    for token in theme.active_tokens() {
        css.push_str(&token.name.value);
        css.push(':');
        css.push_str(&token.value.value.replace("--theme(", "var("));
        css.push(';');
    }
    css.push_str("}}\n");
    css.push_str("@layer theme{");
    css.push_str(&shorth_css::tailwind_v4_keyframes());
    css.push_str("}\n");
    css.push_str("@layer base{");
    css.push_str(&shorth_css::tailwind_v4_preflight());
    css.push('}');
    if !theme.custom_css().trim().is_empty() {
        css.push('\n');
        css.push_str(theme.custom_css());
        css.push('\n');
    }
    css.push_str("\n@layer utilities{");
    css.push_str(utility_css);
    css.push_str("}\n");
    css
}

fn render_react_types(theme: &shorth_css::ThemeIndex) -> String {
    let mut utilities = vec![
        "absolute",
        "block",
        "cursor-pointer",
        "cursor-wait",
        "flex",
        "grid",
        "hidden",
        "inline-block",
        "inline-flex",
        "items-center",
        "items-end",
        "items-start",
        "justify-between",
        "justify-center",
        "opacity-0",
        "opacity-60",
        "opacity-100",
        "overflow-hidden",
        "relative",
        "text-center",
        "text-left",
        "transition",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for token in &theme.tokens {
        let Some((namespace, name)) = token.name.value.trim_start_matches("--").split_once('-')
        else {
            continue;
        };
        for (prefix, expected_namespace) in [
            ("bg", "color"),
            ("text", "color"),
            ("border", "color"),
            ("p", "spacing"),
            ("px", "spacing"),
            ("py", "spacing"),
            ("m", "spacing"),
            ("mx", "spacing"),
            ("my", "spacing"),
            ("gap", "spacing"),
            ("w", "spacing"),
            ("h", "spacing"),
            ("rounded", "radius"),
            ("shadow", "shadow"),
            ("duration", "duration"),
            ("ease", "ease"),
        ] {
            if namespace == expected_namespace {
                utilities.push(format!("{prefix}-{name}"));
            }
        }
    }
    utilities.sort();
    utilities.dedup();
    let entries = utilities
        .into_iter()
        .map(|utility| format!("    \"{utility}\": true;"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut variants = vec![
        "active",
        "dark",
        "disabled",
        "focus",
        "focus-visible",
        "hover",
    ];
    variants.extend(
        theme
            .tokens
            .iter()
            .filter_map(|token| token.name.value.strip_prefix("--breakpoint-")),
    );
    variants.sort_unstable();
    variants.dedup();
    let variants = variants
        .into_iter()
        .map(|variant| format!("    \"{variant}\": true;"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Generated by Stillwind from @theme. Do not edit.\nimport \"stillwind/react\";\n\ndeclare module \"stillwind/react\" {{\n  interface StillwindUtilityVocabulary {{\n{entries}\n  }}\n  interface StillwindVariantVocabulary {{\n{variants}\n  }}\n}}\n\nexport {{}};\n"
    )
}

fn class_name(canonical_input: &str) -> String {
    let hash = blake3::hash(canonical_input.as_bytes());
    format!("_s_{}", &hash.to_hex()[..8])
}

#[cfg(test)]
mod tests {
    use shorth_source::FileId;

    use super::*;

    #[test]
    fn exposes_the_first_frontends_through_one_compiler_api() {
        let js = SourceFile::new(FileId(1), "App.tsx", "export const App = () => <main />;");
        let css = SourceFile::new(FileId(2), "shorth.css", "@theme { --color-accent: red; }");

        assert!(Compiler::check_javascript(&js).is_empty());
        let theme = Compiler::read_theme(&css).unwrap();
        assert_eq!(theme.value("color", "accent"), Some("red"));
        assert!(theme.token("color", "red-500").is_some());
    }

    #[test]
    fn generates_types_from_the_css_theme_vocabulary() {
        let css = SourceFile::new(
            FileId(2),
            "stillwind.css",
            "@theme { --color-accent: red; --spacing-4: 1rem; }",
        );
        let types = Compiler::generate_react_types(&css).unwrap();

        assert!(types.contains("\"bg-accent\": true"));
        assert!(types.contains("\"px-4\": true"));
    }
}
