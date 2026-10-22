import { lintPresets, lintRules, resolveLintConfig, type LintRule } from "stillwind/config";

const presets = {
  recommended: resolveLintConfig(undefined).rules,
  strict: resolveLintConfig({ extends: [lintPresets.strict] }).rules,
  stylistic: resolveLintConfig({ extends: [lintPresets.stylistic] }).rules,
};

export function lintRuleSummary(name: LintRule) {
  if (!(name in lintRules)) {
    throw new Error(`Unknown lint rule: ${name}`);
  }
  const rule = lintRules[name];
  const defaults: Record<string, unknown> = rule.defaults ?? {};
  return {
    name,
    rule,
    levels: Object.entries(presets).map(([preset, rules]) => {
      const value = rules[name];
      return { preset, severity: typeof value === "string" ? value : value.severity };
    }),
    options: (rule.options ?? [])
      .filter((option) => option !== "severity")
      .map((option) => ({ option, value: JSON.stringify(defaults[option] ?? null) })),
  };
}

export const projectAnalysisNote =
  "The CLI and the editor report this rule only when they analyze the whole project.";

export function lintSummaryMarkdown(name: LintRule, analysis?: string | boolean) {
  const { levels, options } = lintRuleSummary(name);
  const table = (head: string, rows: string[][]) =>
    `| ${head} |\n| --- | --- |\n${rows.map((row) => `| ${row.join(" | ")} |`).join("\n")}`;
  return [
    table(
      "Preset | Severity",
      levels.map(({ preset, severity }) => [`\`${preset}\``, severity]),
    ),
    options.length
      ? table(
          "Option | Default",
          options.map(({ option, value }) => [`\`${option}\``, `\`${value}\``]),
        )
      : "",
    "[Configure lint](/docs/configuration/#lint) to change the severity or options.",
    analysis === "project" ? projectAnalysisNote : "",
  ]
    .filter(Boolean)
    .join("\n\n");
}
