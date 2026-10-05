import { fixtureCache } from "@stillwind-tools/workspace/task-cache";
import { defineConfig } from "vite-plus";

const projectCache = {
  ...fixtureCache,
  input: [
    ...fixtureCache.input,
    {
      pattern: "!fixtures/projects/*/expected/**",
      base: "workspace" as const,
    },
    {
      pattern: "!apps/fixtures-viewer/public/fixtures.json",
      base: "workspace" as const,
    },
  ],
};

export default defineConfig({
  root: import.meta.dirname,
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "generate:fixtures:projects": {
        command: "node fixtures.ts",
        cache: projectCache,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
      "test:fixtures:projects": {
        command: "node fixtures.ts --check",
        cache: fixtureCache,
        dependsOn: ["@stillwind/host#build", "@stillwind/native#build"],
      },
    },
  },
});
