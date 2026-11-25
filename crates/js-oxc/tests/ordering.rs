//! Imported values that order CSS between modules, as the module summary reports them.

use std::collections::BTreeMap;

use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, ModuleId, SourceFile};

/// Returns the `(source, imported)` pairs that the summary of `code` reports.
fn ordering_imports(code: &str) -> Vec<(String, String)> {
    let source = SourceFile::new(FileId(1), None, None, code).unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("app.tsx"),
        &[],
        &BTreeMap::new(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    parsed
        .summary
        .ordering_imports
        .iter()
        .map(|import| (import.source.to_string(), import.imported.to_string()))
        .collect()
}

/// Every import that the module reads as a value orders its module first, whatever the
/// value is used for.
#[test]
fn value_imports_order_their_modules_first() {
    let imports = ordering_imports(
        r"import { style } from 'stillwind/react';
import Card from './card';
import * as ui from './ui';
import { Icon } from './icon';
import { base } from './base';
import { unused } from './unused';
import type { Props } from './types';
import { type Shape, wide } from './shapes';
import './side-effect';
const p: Props = { wide };
export const a = <ui.Button {...p}><Icon /><Card /></ui.Button>;
export const b = style(base, 'p-1');
",
    );
    assert_eq!(
        imports,
        [
            ("./base".into(), "base".into()),
            ("./card".into(), "default".into()),
            ("./icon".into(), "Icon".into()),
            ("./shapes".into(), "wide".into()),
            ("./ui".into(), "*".into()),
        ]
    );
}
