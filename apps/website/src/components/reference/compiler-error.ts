export type ErrorCategory = "configuration" | "authoring" | "host" | "internal";

export function compilerErrorMarkdown(code: string, category: ErrorCategory) {
  return `Reporting code: **\`${code}\`**. Category: **${category}**.\n\nThis is a fatal compiler error. Lint settings and suppression comments cannot disable it.\n\n[Reporting-code ledger](https://github.com/SegaraRai/stillwind/blob/main/crates/diagnostics/src/compilation/code.rs) · [Diagnostic payload](https://github.com/SegaraRai/stillwind/blob/main/crates/diagnostics/src/compilation/diagnostic.rs)`;
}
