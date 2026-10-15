//! Display names shared by the source occurrences captured for one editor snapshot.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, LeanString, ToLeanString};

use super::{
    AnchorName, AnchorNameRequest, ClassName, ConditionHelperName, ConditionHelperRequest,
    ConditionRole, ContainerName, ContainerNameRequest, IdentifierHash, IdentifierKind,
    KeyframesName, KeyframesNameRequest, LayerName, LayerNameRequest, NameRequest, RefMarker,
    RefMarkerName, Token, UtilityClass, ViewTransitionClass, ViewTransitionClassRequest,
    ViewTransitionSlotName, ViewTransitionSlotRequest,
};

/// Identifies a request's display role and its original declaration label or binding.
pub(crate) trait PreviewRequest<T: Token>: NameRequest<T> {
    /// Returns the role used in a readable name, or `None` to retain the generated
    /// token. The role is a fixed CSS identifier fragment such as `anchor` or
    /// `if_gate`.
    fn preview_role(&self) -> Option<&'static str>;

    /// Returns the authored label or original binding for a declaration. A missing
    /// label selects a numbered name within the role; an empty label remains a named
    /// request.
    fn preview_label(&self) -> Option<&str> {
        None
    }
}

/// Allocates readable names by identity and keeps distinct declarations
/// distinguishable. Captures share this table; later candidate queries each receive an
/// independent clone.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreviewNames {
    /// Raw tokens by family and full request hash. Aliases of one declaration reuse a
    /// token.
    names: BTreeMap<(IdentifierKind, IdentifierHash), LeanStr>,
    /// Last allocated one-based index for requests without labels, separately per role.
    counters: BTreeMap<&'static str, usize>,
    /// Tokens already allocated across all roles, including names after collision
    /// suffixing.
    used: BTreeSet<LeanStr>,
}

impl PreviewNames {
    /// Returns a raw display token for `request`, allocating it once per family and
    /// hash. Labels retain alphanumeric characters, `_`, and `-`; other characters
    /// become `_`. Unlabeled requests use a one-based role counter, and collisions add
    /// suffixes from `_2`. CSS punctuation such as `.` or `--` is added by the token's
    /// serializer. A request without a display role returns `None` and leaves the table
    /// unchanged.
    pub(crate) fn name<T: Token>(
        &mut self,
        request: &impl PreviewRequest<T>,
    ) -> Option<LeanString> {
        let role = request.preview_role()?;
        let key = (request.kind(), request.hash());
        if let Some(name) = self.names.get(&key) {
            return Some((name.clone()).into());
        }
        let base = if let Some(label) = request.preview_label() {
            let label: LeanString = label
                .chars()
                .map(|character| {
                    if character.is_alphanumeric() || matches!(character, '_' | '-') {
                        character
                    } else {
                        '_'
                    }
                })
                .collect();
            format_args!("__{role}_{label}").to_lean_string()
        } else {
            let index = self.counters.entry(role).or_default();
            *index += 1;
            format_args!("__{role}_{index}").to_lean_string()
        };
        let mut name = base.clone();
        let mut suffix = 2;
        while !self.used.insert((name.clone()).into()) {
            name = format_args!("{base}_{suffix}").to_lean_string();
            suffix += 1;
        }
        self.names.insert(key, (name.clone()).into());
        Some(name)
    }
}

/// Gives declaration requests a fixed role and prefers their label over the original
/// binding.
macro_rules! declaration_roles {
    ($($request:ident : $token:ident = $role:literal;)*) => {
        $(
            impl PreviewRequest<$token> for $request {
                fn preview_role(&self) -> Option<&'static str> {
                    Some($role)
                }

                fn preview_label(&self) -> Option<&str> {
                    Some(self.0.label.as_ref().map_or(self.0.binding.as_str(), |label| label.as_str()))
                }
            }
        )*
    };
}

declaration_roles! {
    RefMarkerName: RefMarker = "ref";
    AnchorNameRequest: AnchorName = "anchor";
    ContainerNameRequest: ContainerName = "container";
    KeyframesNameRequest: KeyframesName = "keyframes";
    ViewTransitionClassRequest: ViewTransitionClass = "transition";
}

impl PreviewRequest<UtilityClass> for ClassName {
    fn preview_role(&self) -> Option<&'static str> {
        None
    }
}

impl PreviewRequest<ConditionHelperName> for ConditionHelperRequest {
    fn preview_role(&self) -> Option<&'static str> {
        Some(match self.0.role {
            ConditionRole::Rule => "rule_boundary",
            ConditionRole::Fallback => "if_fallback",
            ConditionRole::Gate => "if_gate",
            ConditionRole::GateValue => "if_gate_value",
            ConditionRole::GateNot => "if_gate_not",
        })
    }
}

impl PreviewRequest<ViewTransitionSlotName> for ViewTransitionSlotRequest {
    fn preview_role(&self) -> Option<&'static str> {
        Some(if self.0.branch.starts_with("view-transition-name:") {
            "transition_name"
        } else {
            "transition_class"
        })
    }
}

impl PreviewRequest<LayerName> for LayerNameRequest {
    fn preview_role(&self) -> Option<&'static str> {
        Some("layer")
    }
}
