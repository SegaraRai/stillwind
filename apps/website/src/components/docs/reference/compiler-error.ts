export type ErrorCategory = "configuration" | "authoring" | "host" | "internal";

export const issues = "https://github.com/SegaraRai/stillwind/issues";

const nextAction: Record<ErrorCategory, string> = {
  configuration: "Fix `stillwind.config.ts` or the integration options.",
  authoring: "Fix your components, `style()` calls, or stylesheet.",
  host: "Check your integration. With an official integration, update the Stillwind packages, or report a bug.",
  internal: `This is a bug in the compiler. [Report it](${issues}) with the error message.`,
};

export const fatalNote =
  "A compiler error stops the compilation of the module. Lint settings and suppression comments do not affect it.";

export function compilerErrorMarkdown(code: string, category: ErrorCategory) {
  return `Code: **\`${code}\`**. Category: **${category}**. ${nextAction[category]}\n\n${fatalNote}`;
}
