//! Small contribution fixtures shared by the composition and lint unit tests.

use stillwind_diagnostics::LintSettings;
use stillwind_ir::ConditionalBranches;
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{FileId, ModuleId, Span};

use super::{
    DeclarationValue, RuleContext, RuleSubject, StyleEffect, StyleEffects, StyleSupply, SupplyKind,
    UseId,
};

/// Builds a style identity with the same call span in the requested module.
pub(super) fn identity(module: &str) -> IdentityId {
    IdentityId::new(AssetKind::Style, ModuleId::from(module), 0, 100, "")
}

/// Builds a source occurrence whose module and byte offset identify its decision.
pub(super) fn site(module: &str, offset: u32) -> UseId {
    UseId {
        module: ModuleId::from(module),
        span: Span::new(FileId(1), offset, offset + 1),
    }
}

/// Builds an artifact record around a direct contribution tree.
pub(super) fn effects(module: &str, root: StyleEffect) -> StyleEffects {
    StyleEffects {
        lint_settings: LintSettings::with_defaults(),
        identity: identity(module),
        span: Span::new(FileId(1), 0, 100),
        root,
    }
}

/// Builds a supply with ordinal zero at the requested source occurrence.
pub(super) fn supply(module: &str, offset: u32, kind: SupplyKind) -> StyleEffect {
    StyleEffect::Supply(StyleSupply {
        site: site(module, offset),
        kind,
        ordinal: 0,
    })
}

/// Builds a CSS context that affects the styled element without at-rules.
pub(super) fn context(selector: &str) -> RuleContext {
    RuleContext {
        selector: selector.into(),
        at_rules: Vec::new(),
        subject: RuleSubject::Target,
    }
}

/// Builds a declaration with a fixed value and no provider requirement.
pub(super) fn declaration(property: &str, context: RuleContext) -> SupplyKind {
    SupplyKind::Declaration {
        property: property.into(),
        value: DeclarationValue::Css("initial".into()),
        important: false,
        context,
        provider: false,
        gate: None,
    }
}

/// Builds both arms of one source test with a shared decision occurrence.
pub(super) fn branch(module: &str, offset: u32, yes: StyleEffect, no: StyleEffect) -> StyleEffect {
    StyleEffect::Branch {
        condition: site(module, offset),
        branches: ConditionalBranches::Ternary {
            consequent: Box::new(yes),
            alternate: Box::new(no),
        },
    }
}
