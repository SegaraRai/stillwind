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
      "tailwind:extract": {
        command: "node extract-tailwind-v4-registrations.ts",
      },
    },
  },
});
