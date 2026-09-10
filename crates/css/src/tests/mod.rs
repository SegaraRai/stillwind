use stillwind_source::{FileId, SourceFile};

use super::values::webref::webref_property;
use super::*;

mod directives;
mod properties;
mod theme;
mod values;

fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    parse_theme(&crate::css_fixture::source(source.id(), source.path(), source.text()).unwrap())
}
