import { lintPresets, lintRules, resolveLintConfig, type LintRule } from "stillwind/config";

const presets = {
  Default: resolveLintConfig(undefined).rules,
  Strict: resolveLintConfig({ extends: [lintPresets.strict] }).rules,
  "Stylistic alone": resolveLintConfig({ extends: [lintPresets.stylistic] }).rules,
};

export function lintRuleSummary(name: LintRule) {
  if (!(name in lintRules)) {
    throw new Error(`Unknown lint rule: ${name}`);
  }
  return {
    name,
    rule: lintRules[name],
    levels: Object.entries(presets).map(([preset, rules]) => {
      const value = rules[name];
      return { preset, severity: typeof value === "string" ? value : value.severity };
    }),
  };
}

export function lintSummaryMarkdown(name: LintRule, analysis?: string | boolean) {
  const { rule, levels } = lintRuleSummary(name);
  return `Category: **${rule.category}**.\n\n| Preset | Severity |\n| --- | --- |\n${levels.map(({ preset, severity }) => `| ${preset} | ${severity} |`).join("\n")}\n\n${rule.options ? `Option fields: ${rule.options.map((option) => `\`${option}\``).join(", ")}.\n\nDefault detection options:\n\n\`\`\`json\n${JSON.stringify(rule.defaults, null, 2)}\n\`\`\`` : "This rule accepts a severity: `warn`, `deny`, or `off`."}\n\n[Configure lint](/docs/configuration/lint/) to set presets, options, and suppression. Build adapters skip lint detection; the CLI enforces denied findings.${analysis === "project" ? "\n\nThese examples use complete project analysis." : ""}`;
}
