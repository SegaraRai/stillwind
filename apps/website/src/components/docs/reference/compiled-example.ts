import { documentationCompiler, referenceCompiler } from "../../../lib/docs/reference/compiler";

export interface CompiledExampleInput {
  /** TypeScript or TSX module that calls the macros. */
  source?: string;
  /** Project stylesheet text that follows the Stillwind imports. */
  stylesheet?: string;
}

/**
 * Imports the same files as `@import "stillwind"`, except the preflight reset, so
 * that the complete stylesheet of an example shows only the CSS that it selects.
 */
const stylesheetPrefix = `@layer properties, theme, base, components, utilities;
@import "stillwind/theme.css" layer(theme);
@import "stillwind/utilities.css" layer(utilities);
@import "stillwind/builtins.css";
@import "stillwind/templates.css";
`;

const stylesheetCompilers = new Map<string, ReturnType<typeof referenceCompiler>>();

export async function compileExample(input: CompiledExampleInput) {
  if (input.stylesheet === undefined) {
    return compileWith(await documentationCompiler(), input.source!);
  }
  let pending = stylesheetCompilers.get(input.stylesheet);
  if (!pending) {
    pending = referenceCompiler({}, stylesheetPrefix + input.stylesheet);
    stylesheetCompilers.set(input.stylesheet, pending);
  }
  const compiler = await pending;
  const reports = compiler.loader.stylesheetReports();
  if (reports.length) {
    throw new Error(`Documentation stylesheet failed: ${JSON.stringify(reports)}`);
  }
  const { module, css } = compiler.document(input.source);
  if (module && (module.diagnostics.length || !module.code)) {
    throw new Error(`Documentation example failed: ${JSON.stringify(module.diagnostics)}`);
  }
  return {
    stylesheet: input.stylesheet,
    source: input.source,
    code: module?.code,
    css: withoutEmptyBlocks(compiler.query.formatCss(css)).trim(),
  };
}

/**
 * Removes blocks without content, such as a slot's layer when no utility is used,
 * so that an example shows only the CSS that it selects.
 */
function withoutEmptyBlocks(css: string) {
  let previous;
  do {
    previous = css;
    css = css.replace(/^[ \t]*[^{}\n]+\{\s*\}[ \t]*\n?/gm, "");
  } while (css !== previous);
  return css.replace(/\n{3,}/g, "\n\n");
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
    stylesheet: undefined,
    source,
    code: compiled.code,
    css: compiler.query.formatCss(compiled.css).trim(),
  };
}

export type CompiledExampleOutput = Awaited<ReturnType<typeof compileExample>>;

export const noCssMessage = "This macro adds no CSS to the stylesheet.";

/** Labels the stylesheet output, which leaves out the preflight reset. */
export const documentCssHeading = "Generated stylesheet without preflight";

export function compiledExampleMarkdown(example: CompiledExampleOutput, hideCss = false) {
  const fence = (code: string, language: string) =>
    `\n\n\`\`\`${language}\n${code.trim()}\n\`\`\`\n`;
  let markdown = "";
  if (example.stylesheet !== undefined) {
    markdown += `### Stylesheet${fence(example.stylesheet, "css")}\n`;
  }
  if (example.source !== undefined) {
    markdown += `### Source${fence(example.source, "tsx")}\n### Compiled code${fence(example.code!, "ts")}\n`;
  }
  if (hideCss) {
    return markdown;
  }
  if (example.stylesheet !== undefined) {
    return `${markdown}### ${documentCssHeading}${fence(example.css, "css")}`;
  }
  return `${markdown}### Generated CSS\n${example.css ? fence(example.css, "css") : `\n${noCssMessage}\n`}`;
}
