//! The golden demo: one TSX component compiled end to end.
//!
//! Run with `cargo run -p stillwind-compiler --example golden` to print the props
//! replacement, the generated stylesheet, and any diagnostics for the component
//! described in `docs/golden-case.md`. It is a manual inspection aid for reviewing
//! output, not a test.

use stillwind_compiler::Compiler;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceError, SourceFile};

fn main() -> Result<(), SourceError> {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";

declare const loading: boolean;

export function SaveButton() {
  return <button {...style(
    "inline-flex items-center rounded-md bg-accent px-4 text-center",
    loading && "opacity-60",
    { "aria-busy": loading, "data-state": loading ? "loading" : "idle" },
  )}>Save</button>;
}"#,
    )?;
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/stillwind.css",
        r"@theme {
  --color-accent: oklch(.62 .19 255);
  --spacing-4: 1rem;
  --radius-md: .375rem;
}",
    )?;

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    println!("stillwind golden demo\n");
    println!("Props replacement:");
    for transform in &result.transforms {
        println!("  {}", transform.replacement);
    }
    println!("\nGenerated CSS:\n{}", result.css);

    if result.diagnostics.is_empty() {
        println!("Diagnostics: none");
    } else {
        println!("Diagnostics:");
        for diagnostic in &result.diagnostics {
            println!("  {}: {diagnostic}", diagnostic.code());
        }
    }
    Ok(())
}
