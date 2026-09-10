//! JSON harness for real host-converter acceptance tests.

use std::io::{self, Read};

use serde::Deserialize;
use serde_json::json;

use stillwind_source::{
    CoordinateIndex, FileId, ProjectionIndex, ProjectionMap, SourceFile, Span, Utf16Offset,
};

#[derive(Deserialize)]
struct Input {
    name: String,
    generated: String,
    original: String,
    map: ProjectionMap,
    probes: Vec<[u32; 2]>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let inputs: Vec<Input> = serde_json::from_str(&input)?;
    let mut results = Vec::new();
    for input in inputs {
        let generated = SourceFile::new(FileId(2), "generated.tsx", input.generated)?;
        let original = SourceFile::new(FileId(1), "original", input.original)?;
        let coordinates = CoordinateIndex::new(&generated);
        let original_coordinates = CoordinateIndex::new(&original);
        let index = match ProjectionIndex::new(&generated, &original, &input.map) {
            Ok(index) => index,
            Err(error) => {
                results.push(json!({
                    "name": input.name,
                    "error": error.to_string()
                }));
                continue;
            }
        };
        let mut probes = Vec::new();
        for [start, end] in input.probes {
            let span = Span::new(
                generated.id(),
                coordinates.byte(Utf16Offset(start))?.0,
                coordinates.byte(Utf16Offset(end))?.0,
            );
            probes.push(match index.verify_exact(span) {
                Ok(provenance) => json!({
                    "range": [
                        original_coordinates.utf16(provenance.original.start)?.0,
                        original_coordinates.utf16(provenance.original.end)?.0
                    ]
                }),
                Err(error) => json!({
                    "error": error.to_string()
                }),
            });
        }
        results.push(json!({
            "name": input.name,
            "probes": probes
        }));
    }
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
