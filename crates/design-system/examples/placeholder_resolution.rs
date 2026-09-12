//! Small repeatable placeholder hot-path benchmark, without a benchmark dependency.
//! Run with `cargo run --release -p stillwind-design-system --example placeholder_resolution`.

use std::hint::black_box;
use std::time::Instant;

use stillwind_design_system::{parse_theme, resolve_utility};
use stillwind_source::{FileId, SourceFile};

fn main() {
    let repeated = std::iter::repeat_n("--value([length])", 32)
        .collect::<Vec<_>>()
        .join(" + ");
    for (name, candidate, body) in [
        ("plain", "x", "color:red;".to_owned()),
        ("one", "x-[2px]", "width:--value([length]);".to_owned()),
        ("repeated", "x-[2px]", format!("width:calc({repeated});")),
        (
            "mixed",
            "x-[2]/[3]",
            "opacity:calc(--value([number]) * --modifier([number]) + --alpha(.5));".to_owned(),
        ),
    ] {
        let pattern = if name == "plain" { "x" } else { "x-*" };
        let source = SourceFile::new(
            FileId(1),
            "bench.css",
            format!("@utility {pattern} {{ {body} }}"),
        )
        .unwrap();
        let theme = parse_theme(&source).unwrap();
        assert!(resolve_utility(candidate, &theme).is_some());
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..2000 {
                black_box(resolve_utility(black_box(candidate), black_box(&theme)));
            }
            samples.push(start.elapsed().as_nanos() / 2000);
        }
        samples.sort_unstable();
        println!("{name},{} ns/resolve", samples[2]);
    }
}
