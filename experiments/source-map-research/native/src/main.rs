// Benchmark binary and shared algorithms for the source-map research addon.
//
// Two entry points share this file. With `bench-edits` the binary compares a
// naive right-to-left `String::replace_range` edit pass against a single forward
// pass over the same ranges, which is the measurement the experiment exists for.
// Otherwise it reads JSON on stdin and answers with either an encoded source map
// or the emitted code plus map for a batch of edits, so a JavaScript driver can
// check the output against the real consumer contract.
//
// This file is included into `lib.rs`, so it cannot carry inner doc comments:
// the same text is written as regular comments on purpose.
//
// Source-map coordinates count UTF-16 units; edit ranges count UTF-8 bytes. The
// emitter also accepts an unmapped prefix before the edited source.

use std::{
    hint::black_box,
    io::{self, Read},
    time::Instant,
};

use oxc_sourcemap::SourceMapBuilder;
use serde::Deserialize;
use serde_json::json;

use stillwind_source::{BytePos, FileId, SourceFile};

/// One source-map encoding request.
#[derive(Deserialize)]
struct Input {
    /// Optional generated-file name recorded in the map.
    #[serde(default)]
    file: Option<String>,
    /// Source names, parallel to `contents`.
    sources: Vec<String>,
    /// Source texts, so the map can carry `sourcesContent`.
    contents: Vec<String>,
    // generated line, column, optional source, original line, column
    /// Token rows: generated line/column, optional source, original line/column.
    tokens: Vec<(u32, u32, Option<u32>, u32, u32)>,
}

/// Encodes one `Input` into a v3 source map JSON string.
fn encode(input: &Input) -> String {
    let mut builder = SourceMapBuilder::default();
    if let Some(file) = &input.file {
        builder.set_file(file);
    }
    for (name, content) in input.sources.iter().zip(&input.contents) {
        builder.set_source_and_content(name, content);
    }
    for &(line, column, source, original_line, original_column) in &input.tokens {
        builder.add_token(line, column, original_line, original_column, source, None);
    }
    builder.into_sourcemap().to_json_string()
}

/// One replacement-emitter request.
#[derive(Deserialize)]
struct SourceInput {
    /// Original source text the edits apply to.
    source: String,
    /// File name used as both source and generated name.
    id: String,
    /// Text emitted before the source, such as a generated import block.
    prefix: String,
    /// Replacements as `(start, end, text)` over half-open UTF-8 byte ranges in `source`.
    /// Ranges must be ascending, non-overlapping, in bounds, and on scalar boundaries.
    /// Empty ranges insert text; entries at the same empty range retain input order.
    edits: Vec<(usize, usize, String)>,
}

/// Returns edited code and a v3 source-map JSON string for the supplied replacements.
///
/// `input.edits` must satisfy the range contract on [`SourceInput::edits`]. The prefix
/// is unmapped. Unchanged UTF-16 units map to their corresponding original positions;
/// each replacement unit maps to the start of the replaced range. The input is unchanged.
///
/// A token per UTF-16 unit preserves lookups at surrogate halves and newline positions.
/// The empty terminator includes the unchanged suffix in the same emission loop.
///
/// # Panics
///
/// Panics if edit ranges overlap, go backwards, exceed the source, or split UTF-8 scalars.
fn emit(input: &SourceInput) -> (String, String) {
    let mut builder = SourceMapBuilder::default();
    builder.set_file(&input.id);
    let source_id = builder.set_source_and_content(&input.id, &input.source);
    let mut output = input.prefix.clone();
    let (mut gl, mut gc, mut ol, mut oc) = (0, 0, 0, 0);
    for unit in input.prefix.encode_utf16() {
        if unit == 10 {
            gl += 1;
            gc = 0;
        } else {
            gc += 1;
        }
    }
    let mut cursor = 0;
    for (start, end, replacement) in input.edits.iter().chain(std::iter::once(&(
        input.source.len(),
        input.source.len(),
        String::new(),
    ))) {
        assert!(cursor <= *start && start <= end);
        let unchanged = &input.source[cursor..*start];
        output.push_str(unchanged);
        for unit in unchanged.encode_utf16() {
            builder.add_token(gl, gc, ol, oc, Some(source_id), None);
            if unit == 10 {
                gl += 1;
                gc = 0;
                ol += 1;
                oc = 0;
            } else {
                gc += 1;
                oc += 1;
            }
        }
        output.push_str(replacement);
        for unit in replacement.encode_utf16() {
            builder.add_token(gl, gc, ol, oc, Some(source_id), None);
            if unit == 10 {
                gl += 1;
                gc = 0;
            } else {
                gc += 1;
            }
        }
        for unit in input.source[*start..*end].encode_utf16() {
            if unit == 10 {
                ol += 1;
                oc = 0;
            } else {
                oc += 1;
            }
        }
        cursor = *end;
    }
    (output, builder.into_sourcemap().to_json_string())
}

/// Median of `values`, using `total_cmp` so the ordering is total for floats.
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// Applies `ranges` to `source` in both implementations and returns the map.
///
/// The two branches are the experiment: `fused == false` applies the ranges
/// backwards with `replace_range` and then computes offsets, while `fused == true`
/// makes one forward pass and records the mapping as it copies. Both must produce
/// the same output and offsets, which the caller asserts before timing them.
fn edits(
    source: &str,
    ranges: &[(usize, usize, String)],
    fused: bool,
) -> (String, Vec<[usize; 4]>) {
    let file = SourceFile::new(FileId(1), "input.tsx", source).unwrap();
    let mut map = Vec::new();
    if !fused {
        let mut output = source.to_owned();
        for (a, b, text) in ranges.iter().rev() {
            output.replace_range(*a..*b, text);
        }
        let mut delta = 0isize;
        for (a, b, text) in ranges {
            let a = file.utf16_offset(BytePos(*a as u32)).unwrap() as usize;
            let b = file.utf16_offset(BytePos(*b as u32)).unwrap() as usize;
            let start = (a as isize + delta) as usize;
            let replacement_length = text.encode_utf16().count();
            let end = start + replacement_length;
            map.push([a, b, start, end]);
            delta += replacement_length as isize - (b - a) as isize;
        }
        (output, map)
    } else {
        let mut output = String::with_capacity(source.len());
        let (mut byte, mut original, mut generated) = (0, 0, 0);
        for (a, b, text) in ranges {
            let unchanged = &source[byte..*a];
            let width = unchanged.encode_utf16().count();
            original += width;
            generated += width;
            output.push_str(unchanged);
            let original_end = original + source[*a..*b].encode_utf16().count();
            let generated_end = generated + text.encode_utf16().count();
            map.push([original, original_end, generated, generated_end]);
            output.push_str(text);
            byte = *b;
            original = original_end;
            generated = generated_end;
        }
        output.push_str(&source[byte..]);
        (output, map)
    }
}

/// Times both edit implementations over increasing edit counts and prints JSON.
///
/// Each size asserts that the two implementations agree first, so a faster wrong
/// answer cannot be reported as a result.
fn benchmark() {
    let mut results = Vec::new();
    for count in [100, 1000, 5000] {
        let mut source = String::new();
        let mut ranges = Vec::new();
        for i in 0..count {
            source.push_str("// 日本語 🧭\r\nconst value = ");
            let a = source.len();
            source.push_str("style('red')");
            ranges.push((
                a,
                source.len(),
                if i % 3 == 0 {
                    ""
                } else {
                    "{className:\"x\",\nlabel:\"🧭\"}"
                }
                .into(),
            ));
            source.push_str(";\n");
        }
        // Large untouched suffix exposes repeated memmoves.
        source.push_str(&"// untouched tail\n".repeat(10000));
        assert_eq!(
            edits(&source, &ranges, false),
            edits(&source, &ranges, true)
        );
        let mut times = Vec::new();
        for fused in [false, true] {
            let mut samples = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                black_box(edits(black_box(&source), black_box(&ranges), fused));
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.push(median(samples));
        }
        results.push(
            json!({"edits":count,"bytes":source.len(),"baselineMs":times[0],"fusedMs":times[1]}),
        );
    }
    println!("{}", json!(results));
}

/// Dispatches on the subcommand word and otherwise encodes a source map.
fn main() {
    if std::env::args().any(|arg| arg == "bench-edits") {
        benchmark();
        return;
    }
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).unwrap();
    if std::env::args().any(|arg| arg == "emit-batch") {
        let inputs: Vec<SourceInput> = serde_json::from_str(&text).unwrap();
        let results: Vec<_> = inputs
            .iter()
            .map(|input| {
                let (code, map) = emit(input);
                json!({"code":code,"map":serde_json::from_str::<serde_json::Value>(&map).unwrap()})
            })
            .collect();
        println!("{}", json!({"results":results}));
        return;
    }
    if std::env::args().any(|arg| arg == "emit") {
        let input: SourceInput = serde_json::from_str(&text).unwrap();
        let mut times = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            black_box(emit(&input));
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let (code, map) = emit(&input);
        println!(
            "{}",
            json!({"code":code,"map":serde_json::from_str::<serde_json::Value>(&map).unwrap(),"emitMs":median(times)})
        );
        return;
    }
    let input: Input = serde_json::from_str(&text).unwrap();
    let mut times = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        black_box(encode(&input));
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "{}",
        json!({"map":serde_json::from_str::<serde_json::Value>(&encode(&input)).unwrap(),"encodeMs":median(times)})
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every scalar-boundary range pair must give both implementations one answer.
    #[test]
    fn valid_scalar_boundary_edits() {
        let source = "a🧭\r\né中\nxyz";
        let boundaries: Vec<_> = source
            .char_indices()
            .map(|(i, _)| i)
            .chain([source.len()])
            .collect();
        for &a in &boundaries {
            for &b in &boundaries {
                if a <= b {
                    for replacement in ["", "x", "🧭\r\n中"] {
                        let ranges = vec![(a, b, replacement.to_owned())];
                        assert_eq!(edits(source, &ranges, false), edits(source, &ranges, true));
                    }
                }
            }
        }
    }
}
