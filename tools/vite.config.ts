import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
      "webref:generate": {
        command: "node generate-webref-css-domains.mts",
      },
      "webref:check": {
        command: "node generate-webref-css-domains.mts --check",
      },
      "review:refresh": {
        command: "node review-fixtures.mts",
        dependsOn: ["@stillwind/native#build"],
      },
      "review:check": {
        command: "node review-fixtures.mts --check",
        dependsOn: ["@stillwind/native#build"],
      },
      "fixtures:write": {
        command: "node regenerate-tailwind-v4-fixtures.ts --write",
      },
      "fixtures:check": {
        command: "node regenerate-tailwind-v4-fixtures.ts",
      },
      "tailwind:value-grammar": {
        command: "node verify-tailwind-v4-value-grammar.ts",
      },
      "tailwind:parity": {
        command:
          "cargo test -p stillwind-core --test static_compile matches_tailwind_fixture_rules_after_minification",
      },
      "tailwind:parity:all": {
        command: "node tailwind-parity.mts",
        dependsOn: ["@stillwind/native#build"],
      },
      "tailwind:parity:analyze": {
        command: "node tailwind-parity.mts --analyze --quiet --summary-only",
        dependsOn: ["@stillwind/native#build"],
      },
      "tailwind:registry-audit": {
        command:
          "cargo test -p stillwind-tailwind audits_builtin_registry_reachability_from_parity_fixtures -- --ignored --nocapture",
      },
      "tailwind:numeric-domain-audit": {
        command: "node tailwind-numeric-domain-audit.mts",
        dependsOn: ["@stillwind/native#build"],
      },
      "builtins:order": {
        command: "node sort-builtin-utilities.mts",
      },
      "builtins:order:write": {
        command: "node sort-builtin-utilities.mts --write",
      },
      "tailwind:extract": {
        command: "node extract-tailwind-v4-registrations.ts",
      },
    },
  },
});
