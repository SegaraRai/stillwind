use stillwind_design_system::{ResolvedUtility, ThemeIndex, resolve_utility};

pub(crate) struct CandidateResolver;

impl CandidateResolver {
    pub(crate) fn resolve(raw: &str, theme: &ThemeIndex) -> Option<ResolvedUtility> {
        resolve_utility(raw, theme)
    }
}
