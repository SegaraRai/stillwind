//! Runs host-converter outputs through normalization, Oxc and original IR mapping.

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

#[derive(Deserialize)]
struct Input {
    name: String,
    generated: String,
    original: String,
    map: ProjectionMap,
    framework: AdapterKind,
}

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
