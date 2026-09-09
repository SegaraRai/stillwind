use stillwind_core::Compiler;
use stillwind_source::{FileId, SourceFile};

fn main() -> Result<(), stillwind_source::SourceError> {
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

    let result = Compiler::compile_react(&component, &theme);

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
            println!("  {}: {}", diagnostic.code, diagnostic.message);
        }
    }
    Ok(())
}
