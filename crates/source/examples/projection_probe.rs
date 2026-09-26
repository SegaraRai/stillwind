//! JSON harness for real host-converter acceptance tests.
//!
//! Reads an array of cases from stdin - each with a generated text, an original text, a
//! [`ProjectionMap`] and UTF-16 probe spans - and prints one JSON result per case:
//! either the resolved original UTF-16 range or the error `verify_exact` reported.
//! Converters that only exist in a host runtime (TypeScript transforms, Vue and Svelte
//! compilers) are driven through this binary so their real output is checked against the
//! same rules the compiler applies, instead of against a hand-written map.

use std::io::{self, Read};

use serde::Deserialize;
use serde_json::json;

use stillwind_source::{
    CoordinateIndex, FileId, ProjectionIndex, ProjectionMap, SourceFile, Span, Utf16Offset,
};

/// One probe case as supplied by the test driver.
#[derive(Deserialize)]
struct Input {
    /// Label echoed back in the result so a failing case stays identifiable.
    name: String,
    /// Generated (shadow) JavaScript produced by the converter.
    generated: String,
    /// Editable original source the converter projected from.
    original: String,
    /// The map linking the two, in either transport format.
    map: ProjectionMap,
    /// UTF-16 `[start, end]` spans in `generated` to verify.
    probes: Vec<[u32; 2]>,
}

/// Processes every case and prints the results as one JSON array.
///
/// Projection failures are reported per case as data, so one unmappable case does not
/// hide the results of the others; the process fails only on I/O or JSON errors. A
/// probe offset that is not a valid UTF-16 boundary aborts with an error, because the
/// driver supplied a bad probe rather than a bad converter.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let inputs: Vec<Input> = serde_json::from_str(&input)?;
    let mut results = Vec::new();
    for input in inputs {
        let generated = SourceFile::new(FileId(2), None, None, input.generated)?;
        let original = SourceFile::new(FileId(1), None, None, input.original)?;
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
