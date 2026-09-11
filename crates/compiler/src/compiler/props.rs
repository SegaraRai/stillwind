use serde::Serialize;

use stillwind_ir::{AdapterKind, AttributeEntry};
use stillwind_source::{SourceFile, Span};

use crate::runtime::{RuntimeBindings, RuntimeHelper};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ViewTransitionStyleEntry {
    pub(super) value: Option<Span>,
    pub(super) identity: Option<String>,
    pub(super) candidates: Vec<String>,
    pub(super) key: Option<Span>,
}

#[derive(Clone, Debug)]
pub(super) struct DynamicAssetClassEntry {
    pub(super) value: Span,
    pub(super) classes: Vec<(String, String)>,
    pub(super) helper: RuntimeHelper,
}

#[derive(Clone, Copy)]
pub(super) struct PropsParts<'a> {
    pub(super) classes: &'a [GeneratedClass],
    pub(super) attributes: &'a [AttributeEntry],
    pub(super) handle_markers: &'a [String],
    pub(super) external_classes: &'a [Span],
    pub(super) dynamic_handle_classes: &'a [Span],
    pub(super) dynamic_asset_classes: &'a [DynamicAssetClassEntry],
    pub(super) view_transition_style: Option<&'a ViewTransitionStyleEntry>,
}

impl PropsParts<'_> {
    pub(super) fn expressions(&self) -> Vec<Span> {
        self.classes
            .iter()
            .filter_map(|class| class.guard)
            .chain(self.attributes.iter().map(|entry| entry.value))
            .chain(self.external_classes.iter().copied())
            .chain(self.dynamic_handle_classes.iter().copied())
            .chain(self.dynamic_asset_classes.iter().map(|entry| entry.value))
            .chain(
                self.view_transition_style
                    .into_iter()
                    .flat_map(|entry| [entry.value, entry.key].into_iter().flatten()),
            )
            .collect()
    }
}

pub(super) struct ExpressionSource<'a> {
    pub(super) source: &'a SourceFile,
    pub(super) bindings: &'a [(Span, String)],
}

impl ExpressionSource<'_> {
    pub(super) fn get(&self, span: Span) -> Option<&str> {
        if let Some((_, name)) = self.bindings.iter().find(|(at, _)| *at == span) {
            Some(name)
        } else {
            self.source.slice(span).ok()
        }
    }
}

fn guarded_class(source: &ExpressionSource<'_>, class: &GeneratedClass) -> Option<String> {
    let Some(span) = class.guard else {
        return Some(format!("\"{}\"", class.class_name));
    };
    let guard = source.get(span)?;
    let guard = if guard
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    {
        guard.to_owned()
    } else {
        format!("({guard})")
    };
    let negation = if class.guard_when_true { "" } else { "!" };
    Some(format!("{negation}{guard}&&\"{}\"", class.class_name))
}

pub(super) fn render_props(
    source: &ExpressionSource<'_>,
    parts: &PropsParts<'_>,
    adapter: AdapterKind,
    runtime: &mut RuntimeBindings<'_>,
    call_span: Span,
) -> Option<String> {
    let PropsParts {
        classes,
        attributes,
        handle_markers,
        external_classes,
        dynamic_handle_classes,
        dynamic_asset_classes,
        view_transition_style,
    } = *parts;
    let mut entries = Vec::new();
    if !classes.is_empty()
        || !handle_markers.is_empty()
        || !external_classes.is_empty()
        || !dynamic_handle_classes.is_empty()
        || !dynamic_asset_classes.is_empty()
    {
        let mut class_names = handle_markers
            .iter()
            .map(|marker| format!("\"{marker}\""))
            .collect::<Vec<_>>();
        class_names.extend(
            classes
                .iter()
                .map(|class| guarded_class(source, class))
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            external_classes
                .iter()
                .map(|value| source.get(*value).map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_handle_classes
                .iter()
                .map(|value| {
                    source.get(*value).map(|value| {
                        format!(
                            "{}({value})",
                            runtime.require(RuntimeHelper::HandleClass, call_span)
                        )
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_asset_classes
                .iter()
                .map(|entry| {
                    let value = source.get(entry.value)?;
                    let classes = entry
                        .classes
                        .iter()
                        .map(|(identity, class_name)| format!("\"{identity}\":\"{class_name}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    Some(format!(
                        "{}({value},{{{classes}}})",
                        runtime.require(entry.helper, call_span)
                    ))
                })
                .collect::<Option<Vec<_>>>()?,
        );
        let class_value = if external_classes.is_empty() && class_names.len() == 1 {
            class_names.into_iter().next()?
        } else {
            format!(
                "{}({})",
                runtime.require(RuntimeHelper::Clsx, call_span),
                class_names.join(",")
            )
        };
        let property = adapter.spec().class_property;
        entries.push(format!("{property}:{class_value}"));
    }
    for attribute in attributes {
        let value = source.get(attribute.value)?;
        entries.push(format!("\"{}\":{value}", attribute.name.value));
    }
    if let Some(entry) = view_transition_style {
        entries.push(render_view_transition(source, entry, runtime, call_span)?);
    }
    Some(format!("{{{}}}", entries.join(",")))
}

fn render_view_transition(
    source: &ExpressionSource<'_>,
    entry: &ViewTransitionStyleEntry,
    runtime: &mut RuntimeBindings<'_>,
    call_span: Span,
) -> Option<String> {
    let value = match (entry.value, &entry.identity) {
        (Some(value), None) => source.get(value)?.to_owned(),
        (None, Some(identity)) => format!("\"{identity}\""),
        _ => return None,
    };
    let key = match entry.key {
        Some(key) => source.get(key)?.to_owned(),
        None => "undefined".to_owned(),
    };
    let candidates = entry
        .candidates
        .iter()
        .map(|candidate| format!("\"{candidate}\""))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "style:{}({value},{key},[{candidates}],\"--sw-vt-name\",\"--sw-vt-class\")",
        runtime.require(RuntimeHelper::ViewTransitionStyle, call_span)
    ))
}
