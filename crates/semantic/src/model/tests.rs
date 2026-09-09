use super::*;

fn identity(kind: AssetKind, start: u32) -> IdentityId {
    IdentityId::new(kind, r"src\Menu.tsx", start, start + 1, "asset")
}

#[test]
fn identities_are_path_stable_across_operating_systems() {
    assert_eq!(
        IdentityId::new(AssetKind::Ref, r"src\Menu.tsx", 4, 8, "menu"),
        IdentityId::new(AssetKind::Ref, "src/Menu.tsx", 4, 8, "menu")
    );
}

#[test]
fn ref_alternatives_retain_scope_correlation() {
    let scope_a = SymbolicValue::exact(identity(AssetKind::Scope, 1));
    let scope_b = SymbolicValue::exact(identity(AssetKind::Scope, 2));
    let alternatives = [
        RefAlternative::new(identity(AssetKind::Ref, 3), Some(scope_a)).unwrap(),
        RefAlternative::new(identity(AssetKind::Ref, 4), Some(scope_b)).unwrap(),
    ];

    assert_ne!(
        alternatives[0].permitted_scopes,
        alternatives[1].permitted_scopes
    );
}
