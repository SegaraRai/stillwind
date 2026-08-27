//! Public orchestration API for the first compiler slice.

use std::fmt::Write;

use serde::Serialize;
use shorth_grammar::lex_utilities;
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
        shorth_css::parse_theme(source)
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
        for animation in &module.animation_declarations {
            emit_animation_definition(&mut utility_css, animation, &theme, &mut result.diagnostics);
        }
        for transition in &module.view_transition_declarations {
            emit_view_transition_definition(
                &mut utility_css,
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
                    let name = view_transition_css_name(&transition);
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
                                utility.container =
                                    Some(format!("{} ({})", handle_css_name(&handle), query.value));
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
                    let marker = handle_marker_class(&handle);
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
                        declaration: render_handle_projection(&handle, &projection),
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
                            utility.selector.clone_from(&selector);
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
        for handle_declaration in module.handle_declarations {
            result.transforms.push(PropsTransform {
                call_span: handle_declaration,
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

fn animation_css_name(animation: &Spanned<String>) -> String {
    let identity = format!(
        "{}:{}:{}",
        animation.span.file.0, animation.span.start.0, animation.value
    );
    let hash = blake3::hash(identity.as_bytes());
    format!("--sw-a-{}", &hash.to_hex()[..8])
}

fn view_transition_css_name(transition: &Spanned<String>) -> String {
    let identity = format!(
        "{}:{}:{}",
        transition.span.file.0, transition.span.start.0, transition.value
    );
    let hash = blake3::hash(identity.as_bytes());
    format!("--sw-v-{}", &hash.to_hex()[..8])
}

fn emit_animation_definition(
    output: &mut String,
    animation: &AnimationDefinition,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    output.push_str("@keyframes ");
    output.push_str(&animation_css_name(&animation.name));
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
    animation: &Spanned<String>,
    options: &Spanned<String>,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) -> CompiledUtility {
    let mut declaration = format!("animation-name:{};", animation_css_name(animation));
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
    transition: &ViewTransitionDefinition,
    theme: &shorth_css::ThemeIndex,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = view_transition_css_name(&transition.name);
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

fn render_handle_projection(handle: &Spanned<String>, projection: &HandleProjection) -> String {
    let name = handle_css_name(handle);
    match projection {
        HandleProjection::Anchor => format!("anchor-name:{name};"),
        HandleProjection::Position => format!("position-anchor:{name};"),
        HandleProjection::Scope => format!("anchor-scope:{name};"),
        HandleProjection::Container(kind) => {
            format!("container-name:{name};container-type:{};", kind.value)
        }
    }
}

fn handle_css_name(handle: &Spanned<String>) -> String {
    let identity = format!(
        "{}:{}:{}",
        handle.span.file.0, handle.span.start.0, handle.value
    );
    let hash = blake3::hash(identity.as_bytes());
    format!("--sw-h-{}", &hash.to_hex()[..8])
}

fn handle_marker_class(handle: &Spanned<String>) -> String {
    handle_css_name(handle).trim_start_matches("--").to_owned()
}

fn handle_parent_selector(handle: &Spanned<String>, selector: &str) -> String {
    let marker = handle_marker_class(handle);
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
    variants: Vec<String>,
    declaration: String,
    selector: Option<String>,
    container: Option<String>,
}

fn resolve_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<CompiledUtility> {
    let (variants, base) = split_variants(utility);
    let declaration =
        resolve_static_utility(base).or_else(|| resolve_themed_utility(base, theme))?;
    Some(CompiledUtility {
        variants,
        declaration,
        selector: None,
        container: None,
    })
}

fn resolve_static_utility(utility: &str) -> Option<String> {
    const UTILITIES: &[(&str, &str)] = &[
        ("block", "display:block;"),
        ("inline-block", "display:inline-block;"),
        ("contents", "display:contents;"),
        ("flow-root", "display:flow-root;"),
        ("flex", "display:flex;"),
        ("inline-flex", "display:inline-flex;"),
        ("flex-row", "flex-direction:row;"),
        ("flex-col", "flex-direction:column;"),
        ("flex-wrap", "flex-wrap:wrap;"),
        ("flex-1", "flex:1 1 0%;"),
        ("grow", "flex-grow:1;"),
        ("grow-0", "flex-grow:0;"),
        ("shrink", "flex-shrink:1;"),
        ("shrink-0", "flex-shrink:0;"),
        ("grid", "display:grid;"),
        ("hidden", "display:none;"),
        ("visible", "visibility:visible;"),
        ("invisible", "visibility:hidden;"),
        ("place-items-center", "place-items:center;"),
        ("place-content-center", "place-content:center;"),
        ("items-center", "align-items:center;"),
        ("items-start", "align-items:flex-start;"),
        ("items-end", "align-items:flex-end;"),
        ("justify-center", "justify-content:center;"),
        ("justify-between", "justify-content:space-between;"),
        ("justify-start", "justify-content:flex-start;"),
        ("justify-end", "justify-content:flex-end;"),
        ("text-center", "text-align:center;"),
        ("text-left", "text-align:left;"),
        ("font-medium", "font-weight:500;"),
        ("font-semibold", "font-weight:600;"),
        ("font-bold", "font-weight:700;"),
        ("cursor-wait", "cursor:wait;"),
        ("cursor-pointer", "cursor:pointer;"),
        ("relative", "position:relative;"),
        ("absolute", "position:absolute;"),
        ("fixed", "position:fixed;"),
        ("sticky", "position:sticky;"),
        ("static", "position:static;"),
        ("border", "border-width:1px;"),
        ("border-0", "border-width:0;"),
        ("outline-0", "outline-width:0;"),
        ("outline-1", "outline-width:1px;"),
        ("outline-2", "outline-width:2px;"),
        ("outline-offset-2", "outline-offset:2px;"),
        ("position-area-block-end", "position-area:block-end;"),
        ("overflow-hidden", "overflow:hidden;"),
        (
            "transition",
            "transition-property:color,background-color,border-color,outline-color,text-decoration-color,fill,stroke,opacity,box-shadow,transform,filter,backdrop-filter;transition-duration:.15s;transition-timing-function:cubic-bezier(.4,0,.2,1);",
        ),
        ("opacity-0", "opacity:0;"),
        ("opacity-60", "opacity:.6;"),
        ("opacity-100", "opacity:1;"),
        ("translate-x-0", "transform:translateX(0);"),
        ("translate-y-0", "transform:translateY(0);"),
    ];
    if let Some(columns) = utility.strip_prefix("grid-cols-")
        && let Ok(columns) = columns.parse::<u8>()
        && (1..=12).contains(&columns)
    {
        return Some(format!(
            "grid-template-columns:repeat({columns},minmax(0,1fr));"
        ));
    }
    UTILITIES
        .iter()
        .find_map(|(name, declaration)| (*name == utility).then(|| (*declaration).to_owned()))
}

#[allow(clippy::too_many_lines)] // The typed utility registry intentionally stays auditable in one table.
fn resolve_themed_utility(utility: &str, theme: &shorth_css::ThemeIndex) -> Option<String> {
    let (negative, utility) = utility
        .strip_prefix('-')
        .map_or((false, utility), |utility| (true, utility));
    if let Some((color, opacity)) = utility
        .strip_prefix("bg-")
        .and_then(|value| value.rsplit_once('/'))
        && theme.token("color", color).is_some()
    {
        return Some(format!(
            "background-color:color-mix(in oklab,var(--color-{color}) {opacity}%,transparent);"
        ));
    }
    if let Some((prefix, color, opacity)) = ["bg", "text", "border", "outline"]
        .into_iter()
        .find_map(|prefix| {
            utility
                .strip_prefix(&format!("{prefix}-"))
                .and_then(|value| value.rsplit_once('/'))
                .map(|(color, opacity)| (prefix, color, opacity))
        })
        && theme.token("color", color).is_some()
    {
        let property = match prefix {
            "bg" => "background-color",
            "text" => "color",
            "border" => "border-color",
            "outline" => "outline-color",
            _ => unreachable!("only known color utility prefixes are listed"),
        };
        return Some(format!(
            "{property}:color-mix(in oklab,var(--color-{color}) {opacity}%,transparent);"
        ));
    }
    if let Some(token) = utility.strip_prefix("size-")
        && theme.token("spacing", token).is_some()
    {
        return Some(format!(
            "width:var(--spacing-{token});height:var(--spacing-{token});"
        ));
    }
    if let Some(token) = utility.strip_prefix("translate-y-")
        && theme.token("spacing", token).is_some()
    {
        let value = if negative {
            format!("calc(var(--spacing-{token}) * -1)")
        } else {
            format!("var(--spacing-{token})")
        };
        return Some(format!("transform:translateY({value});"));
    }
    if let Some(token) = utility.strip_prefix("translate-x-")
        && theme.token("spacing", token).is_some()
    {
        let value = if negative {
            format!("calc(var(--spacing-{token}) * -1)")
        } else {
            format!("var(--spacing-{token})")
        };
        return Some(format!("transform:translateX({value});"));
    }
    if let Some(token) = utility.strip_prefix("scale-")
        && let Ok(percent) = token.parse::<u16>()
    {
        return Some(format!("transform:scale({});", f64::from(percent) / 100.0));
    }
    for (prefix, namespace, property, supports_negative) in [
        ("bg-", "color", "background-color", false),
        ("text-", "color", "color", false),
        ("border-", "color", "border-color", false),
        ("outline-", "color", "outline-color", false),
        ("p-", "spacing", "padding", false),
        ("px-", "spacing", "padding-inline", false),
        ("py-", "spacing", "padding-block", false),
        ("pt-", "spacing", "padding-top", false),
        ("pr-", "spacing", "padding-right", false),
        ("pb-", "spacing", "padding-bottom", false),
        ("pl-", "spacing", "padding-left", false),
        ("m-", "spacing", "margin", true),
        ("mx-", "spacing", "margin-inline", true),
        ("my-", "spacing", "margin-block", true),
        ("mt-", "spacing", "margin-top", true),
        ("mr-", "spacing", "margin-right", true),
        ("mb-", "spacing", "margin-bottom", true),
        ("ml-", "spacing", "margin-left", true),
        ("gap-", "spacing", "gap", false),
        ("w-", "spacing", "width", false),
        ("h-", "spacing", "height", false),
        ("min-w-", "spacing", "min-width", false),
        ("min-h-", "spacing", "min-height", false),
        ("max-w-", "spacing", "max-width", false),
        ("max-h-", "spacing", "max-height", false),
        ("basis-", "spacing", "flex-basis", false),
        ("rounded-", "radius", "border-radius", false),
        ("shadow-", "shadow", "box-shadow", false),
        ("text-", "text", "font-size", false),
        ("font-", "font", "font-family", false),
        ("leading-", "leading", "line-height", false),
        ("tracking-", "tracking", "letter-spacing", false),
        ("z-", "z-index", "z-index", false),
        ("duration-", "duration", "transition-duration", false),
        ("ease-", "ease", "transition-timing-function", false),
    ] {
        let Some(token) = utility.strip_prefix(prefix) else {
            continue;
        };
        if theme.token(namespace, token).is_some() && (!negative || supports_negative) {
            let value = if negative {
                format!("calc(var(--{namespace}-{token}) * -1)")
            } else {
                format!("var(--{namespace}-{token})")
            };
            return Some(format!("{property}:{value};"));
        }
    }
    None
}

fn split_variants(utility: &str) -> (Vec<String>, &str) {
    let mut depth = 0_u8;
    let mut start = 0;
    let mut variants = Vec::new();
    for (index, character) in utility.char_indices() {
        match character {
            '[' => depth = depth.saturating_add(1),
            ']' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => {
                variants.push(utility[start..index].to_owned());
                start = index + 1;
            }
            _ => {}
        }
    }
    (variants, &utility[start..])
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
        for variant in &utility.variants {
            match variant.as_str() {
                "hover" => selector.push_str(":hover"),
                "focus" => selector.push_str(":focus"),
                "focus-visible" => selector.push_str(":focus-visible"),
                "active" => selector.push_str(":active"),
                "disabled" => selector.push_str(":disabled"),
                "dark" => media.push("(prefers-color-scheme: dark)".to_owned()),
                value if value.starts_with("aria-[") && value.ends_with(']') => {
                    if let Some((name, expected)) = value[6..value.len() - 1].split_once('=') {
                        write!(&mut selector, "[aria-{name}=\"{expected}\"]")
                            .expect("writing to a String cannot fail");
                    }
                }
                value if value.starts_with("data-[") && value.ends_with(']') => {
                    if let Some((name, expected)) = value[6..value.len() - 1].split_once('=') {
                        write!(&mut selector, "[data-{name}=\"{expected}\"]")
                            .expect("writing to a String cannot fail");
                    }
                }
                value => {
                    if let Some(token) = theme.value("breakpoint", value) {
                        media.push(format!("(width >= {token})"));
                    }
                }
            }
        }
        let mut rule = format!("{selector}{{{}}}", combine_declarations(&[utility]));
        for condition in media.into_iter().rev() {
            rule = format!("@media {condition}{{{rule}}}");
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
    let mut css = String::from("@layer theme, base, components, utilities;\n@layer theme{:root{");
    for token in &theme.tokens {
        css.push_str(&token.name.value);
        css.push(':');
        css.push_str(&token.value.value);
        css.push(';');
    }
    css.push_str("}}\n@layer utilities{");
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
        assert_eq!(Compiler::read_theme(&css).unwrap().tokens.len(), 1);
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
