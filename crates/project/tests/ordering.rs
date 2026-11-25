//! CSS order between modules that pass styles to imported components.

mod support;

use support::{analysis, brand_snapshot};

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_ir::{ModuleAnalysis, OrderingModule};
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_source::{ModuleId, SourceRegistry};

fn session() -> ProjectSession {
    ProjectSession::new(
        brand_snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    )
}

/// Returns the analysis of `source` in `name` with each ordering use resolved to
/// `declaring`, as a host would resolve it.
fn ordered(name: &str, source: &str, declaring: &str) -> ModuleAnalysis {
    let mut analysis = analysis(name, source);
    analysis.input.ordering_modules = analysis
        .ir
        .ordering_uses
        .iter()
        .map(|use_| OrderingModule {
            source: use_.source.clone(),
            imported: use_.imported.clone(),
            module_id: ModuleId::from(declaring),
        })
        .collect();
    analysis
}

const BUTTON: &str = r"import { style } from 'stillwind/react';
export function Button(props) {
    return <button {...style('p-10', style.forward(props))} />;
}
";

const APP: &str = r"import { style } from 'stillwind/react';
import { Button } from './ui/button';
export const App = () => <Button {...style('p-4')} />;
";

/// The caller's module path sorts first, but the component's CSS still comes first.
#[test]
fn a_component_precedes_the_module_that_passes_it_a_style() {
    let app = ordered("a/app.tsx", APP, "z/ui/button.tsx");
    assert_eq!(app.ir.ordering_uses.len(), 1);
    assert_eq!(app.ir.ordering_uses[0].imported, "Button");
    let mut session = session();
    session.update_module(analysis("z/ui/button.tsx", BUTTON));
    session.update_module(app);
    let css = session.css().unwrap().stylesheet.css;
    let button = css.find("* 10)").expect("button rule");
    let app = css.find("* 4)").expect("app rule");
    assert!(button < app, "{css}");
}

/// The import alone orders the component first, even when the caller's style reaches
/// the component through a variable or a child element.
#[test]
fn an_imported_component_precedes_the_importer_however_the_style_reaches_it() {
    let app = r"import { style } from 'stillwind/react';
import { Button } from './ui/button';
const wide = style('p-4');
export const App = () => <Button asChild><a {...wide} /></Button>;
";
    let mut session = session();
    session.update_module(analysis("z/ui/button.tsx", BUTTON));
    session.update_module(ordered("a/app.tsx", app, "z/ui/button.tsx"));
    let css = session.css().unwrap().stylesheet.css;
    let button = css.find("* 10)").expect("button rule");
    let app = css.find("* 4)").expect("app rule");
    assert!(button < app, "{css}");
}

/// In a chain of wrapper components, each component's CSS comes before the CSS
/// of the module that passes it a style, although the paths sort the other way.
#[test]
fn a_wrapper_chain_orders_each_component_before_its_caller() {
    let fancy = r"import { style } from 'stillwind/react';
import { Button } from '../z/ui/button';
export function FancyButton(props) {
    return <Button {...style('p-8', style.forward(props))} />;
}
";
    let app = r"import { style } from 'stillwind/react';
import { FancyButton } from '../m/fancy-button';
export const App = () => <FancyButton {...style('p-4')} />;
";
    let mut session = session();
    session.update_module(analysis("z/ui/button.tsx", BUTTON));
    session.update_module(ordered("m/fancy-button.tsx", fancy, "z/ui/button.tsx"));
    session.update_module(ordered("a/app.tsx", app, "m/fancy-button.tsx"));
    let css = session.css().unwrap().stylesheet.css;
    let button = css.find("* 10)").expect("button rule");
    let fancy = css.find("* 8)").expect("wrapper rule");
    let app = css.find("* 4)").expect("app rule");
    assert!(button < fancy && fancy < app, "{css}");
}

/// Ordering modules that form a cycle keep one order and report the cycle.
#[test]
fn an_ordering_cycle_is_reported_without_failing() {
    let button = r"import { style } from 'stillwind/react';
import { Icon } from '../../a/app';
export function Button(props) {
    return <Icon {...style('p-10', style.forward(props))} />;
}
";
    let mut session = session();
    session.update_module(ordered("z/ui/button.tsx", button, "a/app.tsx"));
    session.update_module(ordered("a/app.tsx", APP, "z/ui/button.tsx"));
    assert!(session.css().is_ok());
    let findings = session
        .artifacts()
        .get(&ModuleId::from("a/app.tsx"))
        .expect("published")
        .lint_findings
        .iter()
        .filter(|finding| finding.code() == "cyclic-style-order")
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{findings:?}");
    let message = findings[0].diagnostic.to_string();
    assert!(message.contains("z/ui/button.tsx"), "{message}");
}
