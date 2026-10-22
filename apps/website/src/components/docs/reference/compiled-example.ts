import { documentationCompiler } from "../../../lib/docs/reference/compiler";

export async function compileExample(source: string) {
  const compiler = await documentationCompiler();
  return compileWith(compiler, source);
}

export function compileWith(
  compiler: Awaited<ReturnType<typeof documentationCompiler>>,
  source: string,
) {
  const compiled = compiler.compile(source);
  if (compiled.diagnostics.length || !compiled.code) {
    throw new Error(`Documentation example failed: ${JSON.stringify(compiled.diagnostics)}`);
  }
  return {
    source,
    code: compiled.code,
    css: compiler.query.formatCss(compiled.css).trim(),
  };
}

export const noCssMessage = "This macro adds no CSS to the stylesheet.";

export function compiledExampleMarkdown(example: ReturnType<typeof compileWith>) {
  const fence = (code: string, language: string) =>
    `\n\n\`\`\`${language}\n${code.trim()}\n\`\`\`\n`;
  return `### Source${fence(example.source, "tsx")}\n### Compiled code${fence(example.code, "ts")}\n### Generated CSS\n${example.css ? fence(example.css, "css") : `\n${noCssMessage}\n`}`;
}
