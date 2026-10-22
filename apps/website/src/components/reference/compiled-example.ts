import { documentationCompiler } from "../../lib/reference/compiler";

export async function compileExample(source: string) {
  const compiler = await documentationCompiler();
  return compileWith(compiler, source);
}

export function compileWith(
  compiler: Awaited<ReturnType<typeof documentationCompiler>>,
  source: string,
) {
  const development = compiler.compile(source, "development");
  const production = compiler.compile(source);
  if (development.diagnostics.length || production.diagnostics.length || !production.code) {
    throw new Error(
      `Documentation example failed: ${JSON.stringify([...development.diagnostics, ...production.diagnostics])}`,
    );
  }
  return {
    source,
    development: development.code ?? "",
    production: production.code,
    css: compiler.query.formatCss(production.css),
    documentCss: production.css ? compiler.query.formatCss(production.documentCss) : "",
  };
}

export function compiledExampleMarkdown(example: Awaited<ReturnType<typeof compileExample>>) {
  const fence = (code: string, language: string) =>
    `\n\n\`\`\`${language}\n${code.trim()}\n\`\`\`\n`;
  return `### Source${fence(example.source, "tsx")}\n### Development JavaScript${fence(example.development, "ts")}\n### Production JavaScript${fence(example.production, "ts")}\n### Module CSS\n${example.css ? fence(example.css, "css") : "This example contributes no document CSS. Exported CSS text appears in its JavaScript string."}\n${example.documentCss ? `### Complete document stylesheet${fence(example.documentCss, "css")}` : ""}`;
}
