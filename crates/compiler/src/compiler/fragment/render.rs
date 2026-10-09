//! JavaScript materialization of selected style-fragment plans.
//!
//! [`render_style_call`] selects direct framework props before and after folding.
//! Remaining symbolic contributions use the merge runtime. Source-owned fragments
//! retain occurrence mappings; synthetic keys and helpers receive the enclosing
//! transform's fallback location. The caller supplies finalized CSS identities and
//! validates the macro contract.

use stillwind_ir::{ConditionalBranches, FrameworkSpec, StyleFormat};
use stillwind_source::Span;

use super::super::expression_render::{collect_class_arguments, render_plain};
use super::super::props::StyleEmission;
use super::expression::{Atom, Emitted, Fragment, Property, ValueExpr};
use super::fold::{Demand, Field, fold, known_record, record_fields, static_class};
use crate::generated::{Code, code, js_string};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// Materializes a finalized style call as one framework-props expression.
///
/// The emission must satisfy the macro purity contract and the class-atom invariants
/// of [`Fragment::from_emission`]. Layout selection can reorder evaluations and omit
/// empty contributions. Direct props may retain empty class or undefined style fields.
/// Registers helpers only for the chosen output. Retained dynamic code keeps its source
/// mappings, while folded literal text is generated syntax.
pub(in super::super) fn render_style_call(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
) -> Code {
    let source = Fragment::from_emission(emission);
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    if let Some(atoms) = known_record(&source)
        && let Some(fields) = record_fields(&atoms, Demand::Props, fw)
    {
        return renderer.record(&atoms, &fields, Demand::Props);
    }
    let folded = fold(source, fw);
    if let Some(atoms) = known_record(&folded)
        && let Some(fields) = record_fields(&atoms, Demand::Props, fw)
    {
        return renderer.record(&atoms, &fields, Demand::Props);
    }
    if let Some(result) = renderer.style_props(&folded) {
        return result;
    }
    let arguments = match &folded {
        Fragment::Sequence(children) => children
            .iter()
            .map(|child| renderer.fragment(child))
            .collect::<Vec<_>>(),
        other => vec![renderer.fragment(other)],
    };
    let helper = renderer
        .runtime
        .require(RuntimeHelper::Merge, renderer.span);
    code![helper, "(", Code::join(arguments, ","), ")"]
}

/// Renders one CVA value as class text or private fragments for runtime selection.
/// Inputs must contain only classes and custom properties and satisfy the macro purity contract.
pub(in super::super) fn render_cva_value(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
) -> Code {
    let folded = fold(Fragment::from_emission(emission), fw);
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    if let Some(atoms) = known_record(&folded)
        && atoms.iter().all(|atom| matches!(atom, Atom::Class(_)))
    {
        let classes = atoms
            .iter()
            .filter_map(|atom| match atom {
                Atom::Class(value) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(names) = classes
            .iter()
            .map(|value| static_class(value))
            .collect::<Option<Vec<_>>>()
        {
            return js_string(
                &names
                    .into_iter()
                    .filter(|name| !name.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .into();
        }
        return renderer.classes(&classes);
    }
    renderer.fragment(&folded)
}

/// Per-call rendering context shared by selected fragment subtrees.
struct Renderer<'a, 'b> {
    /// Final property names, style format, and runtime conventions.
    fw: &'a FrameworkSpec,
    /// Binding allocator mutated when rendered syntax uses a helper.
    runtime: &'a mut RuntimeBindings<'b>,
    /// Original call span used when a synthetic helper has no more specific origin.
    span: Span,
}

impl Renderer<'_, '_> {
    /// Renders a subtree for consumption by the fragment runtime.
    ///
    /// Class atoms must satisfy the invariants of [`Fragment::from_emission`]. An empty
    /// record becomes `null`, and records that cannot share fields become arrays of
    /// singleton fragments. Registers helpers for emitted branches and values.
    ///
    /// # Panics
    ///
    /// Panics if a retained class argument violates the contract of [`Self::classes`].
    fn fragment(&mut self, fragment: &Fragment) -> Code {
        match fragment {
            Fragment::Record(atoms) => {
                if atoms.is_empty() {
                    return "null".into();
                }
                match record_fields(atoms, Demand::Fragment, self.fw) {
                    Some(fields) => self.record(atoms, &fields, Demand::Fragment),
                    None => code![
                        "[",
                        Code::join(
                            atoms
                                .iter()
                                .map(|atom| match atom {
                                    Atom::Attribute(p) => code![
                                        "{",
                                        Self::property(p, Code::source(p.value.clone(), p.span)),
                                        "}"
                                    ],
                                    Atom::Class(_) => self.record(
                                        std::slice::from_ref(atom),
                                        &[Field::Classes(vec![0])],
                                        Demand::Fragment
                                    ),
                                    Atom::Custom(_) => self.record(
                                        std::slice::from_ref(atom),
                                        &[Field::Custom(vec![0])],
                                        Demand::Fragment
                                    ),
                                })
                                .collect::<Vec<_>>(),
                            ","
                        ),
                        "]"
                    ],
                }
            }
            Fragment::Sequence(children) => code![
                "[",
                Code::join(
                    children
                        .iter()
                        .map(|child| self.fragment(child))
                        .collect::<Vec<_>>(),
                    ","
                ),
                "]"
            ],
            Fragment::Branch { test, branches } => match branches {
                ConditionalBranches::And { consequent } => {
                    code!["(", test, ") && (", self.fragment(consequent), ")"]
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    code![
                        "(",
                        test,
                        ") ? (",
                        self.fragment(consequent),
                        ") : (",
                        self.fragment(alternate),
                        ")"
                    ]
                }
            },
            Fragment::FromProps(expression) => {
                let helper = self.runtime.require(RuntimeHelper::ToFragment, self.span);
                code![helper, "((", expression, "))"]
            }
            Fragment::StylePayload(expression) => {
                let key = self.runtime.require(RuntimeHelper::StyleKey, self.span);
                code!["{[", key, "]:", expression.render(self.runtime), "}"]
            }
        }
    }

    /// Wraps the entries from [`Self::record_entries`] in an object literal.
    /// Requires that method's field-layout preconditions and registers its selected helpers.
    ///
    /// # Panics
    ///
    /// Panics under the conditions documented by [`Self::record_entries`].
    fn record(&mut self, atoms: &[Atom], fields: &[Field], demand: Demand) -> Code {
        let entries = self.record_entries(atoms, fields, demand);
        code!["{", Code::join(entries, ","), "}"]
    }

    /// Renders fields in layout order without object braces, omitting empty static
    /// classes.
    ///
    /// Indices must address matching atom variants in this exact slice. The layout must
    /// come from [`record_fields`] for the same demand and framework, or describe a
    /// single fragment channel. Reusing indices after changing the atoms can select
    /// wrong values. Class atoms must satisfy [`Self::classes`]' structural
    /// precondition. Registers helpers for emitted values and symbolic keys.
    ///
    /// # Panics
    ///
    /// Panics for out-of-bounds indices or a retained class argument that
    /// [`Self::classes`] cannot render.
    fn record_entries(&mut self, atoms: &[Atom], fields: &[Field], demand: Demand) -> Vec<Code> {
        let mut entries = Vec::new();
        for field in fields {
            match field {
                Field::Attribute(i) => {
                    if let Atom::Attribute(p) = &atoms[*i] {
                        entries.push(Self::property(p, Code::source(p.value.clone(), p.span)));
                    }
                }
                Field::Classes(indices) => {
                    let values = indices
                        .iter()
                        .filter_map(|i| match &atoms[*i] {
                            Atom::Class(value) => Some(value),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    if let Some(entry) = self.class_field(&values, demand) {
                        entries.push(entry);
                    }
                }
                Field::Custom(indices) => {
                    let properties = indices
                        .iter()
                        .filter_map(|i| match &atoms[*i] {
                            Atom::Custom(p) => Some(p),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let value =
                        if demand == Demand::Props && self.fw.style_format == StyleFormat::String {
                            self.custom_style_string(&properties)
                        } else {
                            code![
                                "{",
                                Code::join(
                                    properties
                                        .iter()
                                        .map(|p| Self::property(p, self.custom_value(p)))
                                        .collect::<Vec<_>>(),
                                    ","
                                ),
                                "}"
                            ]
                        };
                    let key = if demand == Demand::Props {
                        self.fw.style_property.into()
                    } else {
                        format!(
                            "[{}]",
                            self.runtime
                                .require(RuntimeHelper::CustomPropertiesKey, self.span)
                        )
                    };
                    entries.push(code![key, ":", value]);
                }
            }
        }
        entries
    }

    /// Renders one symbolic or framework class field from ordered class contributions.
    ///
    /// Returns `None` without registering helpers when the slice is empty or consists
    /// only of empty static names. Dynamic values retain the field even when empty at
    /// runtime. A lone opaque expression is emitted directly, so its caller must
    /// guarantee a value valid for the selected consumer. Other trees must satisfy
    /// [`Self::classes`].
    ///
    /// # Panics
    ///
    /// Panics if a retained class argument violates the contract of [`Self::classes`].
    fn class_field(&mut self, values: &[&Emitted], demand: Demand) -> Option<Code> {
        let value = if let Some(strings) = values
            .iter()
            .map(|value| static_class(value))
            .collect::<Option<Vec<_>>>()
        {
            let text = strings
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() {
                return None;
            }
            js_string(&text).into()
        } else if values.len() == 1 {
            if let Emitted::Expression(expression) = values[0] {
                code!["(", expression.render(self.runtime), ")"]
            } else {
                self.classes(values)
            }
        } else {
            self.classes(values)
        };
        let key = if demand == Demand::Props {
            self.fw.class_property.into()
        } else {
            format!(
                "[{}]",
                self.runtime.require(RuntimeHelper::ClassKey, self.span)
            )
        };
        Some(code![key, ":", value])
    }

    /// Attempts direct props for one whole-style payload and representable record
    /// siblings.
    ///
    /// The payload must be a direct child of the input sequence or the input itself.
    /// Returns `None` without registering helpers for missing or multiple payloads,
    /// non-record siblings, custom-property overlays, an ordinary framework style field,
    /// or a rejected props layout. Success registers helpers and appends the framework
    /// style field after the record fields. Unknown payloads use runtime format conversion
    /// and can leave that field present with an undefined value.
    ///
    /// # Panics
    ///
    /// Panics if a retained class argument violates the contract of [`Self::classes`].
    fn style_props(&mut self, fragment: &Fragment) -> Option<Code> {
        let children = match fragment {
            Fragment::Sequence(children) => children.as_slice(),
            other => std::slice::from_ref(other),
        };
        let mut atoms = Vec::new();
        let mut payload = None;
        for child in children {
            match child {
                Fragment::StylePayload(value) if payload.is_none() => payload = Some(value),
                other => atoms.extend(known_record(other)?),
            }
        }
        let payload = payload?;
        if atoms.iter().any(|atom| {
            matches!(atom, Atom::Custom(_))
                || matches!(atom, Atom::Attribute(p) if p.name == self.fw.style_property)
        }) {
            return None;
        }
        let fields = record_fields(&atoms, Demand::Props, self.fw)?;
        let style = match payload {
            ValueExpr::Entry { property, value }
                if property != "__proto__"
                    && (self.fw.style_format == StyleFormat::Object
                        || matches!(
                            value.as_ref(),
                            ValueExpr::Call {
                                helper: RuntimeHelper::ViewTransitionName
                                    | RuntimeHelper::CssValue
                                    | RuntimeHelper::Clsx,
                                ..
                            }
                        )) =>
            {
                let value = if self.fw.style_format == StyleFormat::String {
                    code![
                        js_string(&format!("{property}:")),
                        "+(",
                        value.render(self.runtime),
                        ")"
                    ]
                } else {
                    payload.render(self.runtime)
                };
                code![self.fw.style_property, ":", value]
            }
            _ => {
                let helper = self.runtime.require(RuntimeHelper::StyleValue, self.span);
                code![
                    self.fw.style_property,
                    ":",
                    helper,
                    "(",
                    payload.render(self.runtime),
                    ")"
                ]
            }
        };
        let mut entries = self.record_entries(&atoms, &fields, Demand::Props);
        entries.push(style);
        Some(code!["{", Code::join(entries, ","), "}"])
    }

    /// Normalizes ordered class contributions into a string-valued expression.
    ///
    /// Input trees must contain only class contributions. Argument collection can drop
    /// empty trees and flatten arrays, class groups, and deferred clsx calls under the
    /// macro purity contract. Returns `""` without registering helpers when no
    /// arguments remain. Otherwise registers clsx and any deferred helpers, retaining
    /// dynamic code mappings.
    ///
    /// # Panics
    ///
    /// Panics when a retained argument contains a group that [`render_plain`] rejects,
    /// including a property-bearing group or a group with no retained class entries.
    #[allow(clippy::expect_used)] // Atom::Class is constructed only after render_plain succeeds.
    fn classes(&mut self, values: &[&Emitted]) -> Code {
        let mut arguments = Vec::new();
        for value in values {
            collect_class_arguments(value, &mut arguments);
        }
        let values = arguments;
        if values.is_empty() {
            return js_string("").into();
        }
        let helper = self.runtime.require(RuntimeHelper::Clsx, self.span);
        code![
            helper,
            "(",
            Code::join(
                values
                    .iter()
                    .flat_map(|value| match value {
                        Emitted::Expression(ValueExpr::Call {
                            helper: RuntimeHelper::Clsx,
                            arguments,
                            ..
                        }) => arguments.clone(),
                        _ => vec![render_plain(value, self.runtime).expect("class-only tree")],
                    })
                    .collect::<Vec<_>>(),
                ","
            ),
            ")"
        ]
    }

    /// Serializes ordered custom properties as a JavaScript string expression.
    ///
    /// Names must be unique and valid CSS property names. Values must satisfy
    /// [`Self::custom_value`]'s conversion contract. Joins declarations with semicolons
    /// without CSS escaping, coalescing static text around dynamic conversions. Empty input
    /// produces `""`. Only dynamic values register conversion helpers and retain mappings.
    fn custom_style_string(&mut self, properties: &[&Property]) -> Code {
        let mut text = String::new();
        let mut parts = Vec::new();
        for (index, property) in properties.iter().enumerate() {
            if index > 0 {
                text.push(';');
            }
            text.push_str(&property.name);
            text.push(':');
            if let Some(value) = &property.static_value {
                text.push_str(value);
            } else {
                parts.push(js_string(&text).into());
                text.clear();
                parts.push(self.custom_value(property));
            }
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(js_string(&text).into());
        }
        Code::join(parts, "+")
    }

    /// Produces a JavaScript expression for one custom property's CSS text.
    ///
    /// `static_value`, when present, must already match the runtime CSS-value
    /// conversion of `value`. It is trusted and emitted without a helper or an exact
    /// source mapping. Otherwise `value` must match the source at `span` and yield an
    /// accepted CSS-value input. Registers the conversion helper in that span's import
    /// realm and maps the original expression.
    fn custom_value(&mut self, p: &Property) -> Code {
        if let Some(value) = &p.static_value {
            return js_string(value).into();
        }
        if let Some(value) = &p.computed_value {
            return value.render(self.runtime);
        }
        let helper = self.runtime.require(RuntimeHelper::CssValue, p.span);
        code![helper, "(", Code::source(p.value.clone(), p.span), ")"]
    }

    /// Builds an object-literal entry with an already-converted value expression.
    ///
    /// Preserves the value's mappings and generates the quoted key without CSS
    /// conversion. Uses ordinary key syntax, so `__proto__` retains JavaScript's
    /// prototype-setter semantics.
    fn property(p: &Property, value: Code) -> Code {
        code![js_string(&p.name), ":", value]
    }
}

/// Builds runtime materialization without combining adjacent records for comparison
/// tests. Uses the same emission-to-fragment conversion and leaf renderer as optimized
/// code, but always retains the outer merge. Helper bindings are allocated in
/// `runtime`.
#[cfg(test)]
pub(super) fn render_reference(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
) -> Code {
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    let fragment = renderer.fragment(&Fragment::from_emission(emission));
    let merge = renderer
        .runtime
        .require(RuntimeHelper::Merge, emission.call_span);
    code![merge, "(", fragment, ")"]
}
