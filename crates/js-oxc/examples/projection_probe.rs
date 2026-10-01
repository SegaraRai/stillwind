//! Runs host-converter outputs through normalization, Oxc and original IR mapping.
//!
//! Reads an array of cases from stdin, each with the generated text, the original text,
//! a projection map and a framework, then prints one JSON result per case: the
//! normalized analysis source, the mapped IR, the collected provenance, the
//! diagnostics, or the error that stopped the conversion.
//!
//! Every case also runs the same input through `OxcSourceAnalyzer` and asserts that the
//! direct normalization and the analyzer's projected scan agree, so the harness checks
//! the two code paths against each other rather than only reporting what each one
//! produced.

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use stillwind_ir::{
    AdapterKind, ConverterIdentity, PreparedSource, ProjectedSource, ScriptLanguage,
    ScriptModuleKind, ScriptSyntax, SourceAnalyzer, SourceDocument, SourceFormat, SourceInput,
    SourceProjection,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_source::{FileId, ModuleId, ProjectionMap, SourceFile};

/// One converter-output case as supplied by the test driver.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Input {
    /// Label echoed back in the result so a failing case stays identifiable.
    name: String,
    /// Shadow module text the converter produced.
    generated: String,
    /// Editable original the converter projected from.
    original: String,
    /// The map linking the two, in either transport format.
    map: ProjectionMap,
    /// Framework whose scaffolding rules apply.
    framework: AdapterKind,
}

/// Processes every case and prints the results as one JSON array.
///
/// A failed normalization is reported per case as data, so one bad converter output
/// does not hide the rest; the process fails only on I/O or JSON errors.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let inputs: Vec<Input> = serde_json::from_str(&input)?;
    let mut results = Vec::new();
    for input in inputs {
        let generated = SourceFile::new(FileId(2), None, None, input.generated)?;
        let original = SourceFile::new(FileId(1), None, None, input.original)?;
        let normalized = match normalize_projection(
            &generated,
            &original,
            &input.map,
            input.framework,
            FileId(3),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ) {
            Ok(value) => value,
            Err(error) => {
                results.push(json!({"name":input.name,"error":error.to_string()}));
                continue;
            }
        };
        let parsed = parse(
            &normalized.source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        let format = match input.framework {
            AdapterKind::Astro => SourceFormat::Astro,
            AdapterKind::Svelte => SourceFormat::Svelte,
            AdapterKind::Vue => SourceFormat::Vue,
            _ => {
                results.push(json!({
                    "name": input.name,
                    "error": "projected inputs require a component adapter",
                }));
                continue;
            }
        };
        let projection = match SourceProjection::try_new(
            Arc::new(generated.clone()),
            ScriptSyntax::new(ScriptLanguage::Tsx, ScriptModuleKind::Unambiguous),
            input.map.clone(),
            ConverterIdentity {
                name: "real-converter-fixture".into(),
                version: "pinned".into(),
                options_fingerprint: "research".into(),
            },
            FileId(3),
            Vec::new(),
            Vec::new(),
        )
        .and_then(|projection| {
            ProjectedSource::try_new(
                SourceDocument::new(Arc::new(original.clone()), format, None),
                projection,
            )
        }) {
            Ok(projected) => projected,
            Err(error) => {
                results.push(json!({
                    "name": input.name,
                    "error": error.to_string(),
                }));
                continue;
            }
        };
        let mut source = SourceInput::new(
            ModuleId::from(input.name.as_str()),
            PreparedSource::Projected(projection),
        );
        source.framework = Some(input.framework);
        let scan = OxcSourceAnalyzer.scan_input(source);
        let analysis = OxcSourceAnalyzer.finalize(scan, None);
        match normalized.original_ir(&parsed.module) {
            Ok((ir, provenance)) => {
                assert_eq!(ir, analysis.ir, "projected analyzer and direct normalization must agree");
                results.push(json!({"name":input.name,"ir":analysis.ir,"provenance":provenance,"diagnostics":analysis.diagnostics,"normalized":normalized.source.text()}));
            },
            Err(error) => results.push(json!({"name":input.name,"error":error.to_string(),"normalized":normalized.source.text()})),
        }
    }
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
