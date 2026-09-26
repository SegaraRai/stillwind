//! `stillwind/module-css` import recognition.
//!
//! The module-css import is a value import: a type-only binding must not count,
//! while a mixed clause with at least one value specifier must. Spans must cover
//! the literal including its quotes, because the host rewrites that exact range.

use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, SourceFile};

/// Only value imports of the module-css entry point are recorded.
#[test]
fn explicit_module_css_tracks_value_import_literals_only() {
    for (text, expected) in [
        ("import css from 'stillwind/module-css';", 1),
        ("import * as css from 'stillwind/module-css';", 1),
        ("import 'stillwind/module-css';", 1),
        ("import {} from 'stillwind/module-css';", 1),
        ("import type Css from 'stillwind/module-css';", 0),
        (
            "import {type default as Css} from 'stillwind/module-css';",
            0,
        ),
        (
            "import {type Foo, default as css} from 'stillwind/module-css';",
            1,
        ),
        ("const text = 'stillwind/module-css';", 0),
        ("import css from 'stillwind/module\\u002dcss';", 1),
    ] {
        let source = SourceFile::new(FileId(1), None, None, text).unwrap();
        let parsed = parse(
            &source,
            "test.tsx",
            &[],
            &std::collections::BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty(), "{text}");
        assert_eq!(parsed.module.imports.module_css.len(), expected, "{text}");
        for span in parsed.module.imports.module_css {
            let literal = source.slice(span).unwrap();
            assert!(
                literal.starts_with('\'') && literal.ends_with('\''),
                "{literal}"
            );
        }
    }
}
