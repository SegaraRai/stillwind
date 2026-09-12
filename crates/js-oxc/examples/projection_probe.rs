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

use std::io::{self, Read};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use stillwind_ir::{
    AdapterKind, ConverterIdentity, FrameworkProjection, ScriptLanguage, SourceAnalyzer,
    SourceInput,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_source::{FileId, ProjectionMap, SourceFile};

/// One converter-output case as supplied by the test driver.
#[derive(Deserialize)]
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
        let generated = SourceFile::new(FileId(2), "generated.tsx", input.generated)?;
        let original = SourceFile::new(FileId(1), "original", input.original)?;
        let normalized = match normalize_projection(
            &generated,
            &original,
            &input.map,
            input.framework,
            FileId(3),
        ) {
            Ok(value) => value,
            Err(error) => {
                results.push(json!({"name":input.name,"error":error.to_string()}));
                continue;
            }
        };
        let parsed = parse(&normalized.source);
        let scan = OxcSourceAnalyzer.scan_input(SourceInput {
            code: Arc::new(generated.clone()),
            language: ScriptLanguage::Tsx,
            framework: Some(input.framework),
            upstream_map: None,
            stylesheet_imports: Vec::new(),
            module_css_import: None,
            projection: Some(FrameworkProjection {
                original: Arc::new(original.clone()),
                map: input.map.clone(),
                converter: ConverterIdentity {
                    name: "real-converter-fixture".into(),
                    version: "pinned".into(),
                    options_fingerprint: "research".into(),
                },
                analysis_id: FileId(3),
                realms: Vec::new(),
                regions: Vec::new(),
            }),
        });
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
