use stillwind_source::{FileId, SourceFile};

use crate::*;

mod theme;

fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    parse_theme(&crate::css_fixture::source(source.id(), source.path(), source.text()).unwrap())
}
