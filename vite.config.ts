import { defineConfig } from "vite-plus";

export default defineConfig({
  lint: {
    ignorePatterns: [
      "fixtures/projects/*/expected/**",
      "apps/fixtures-viewer/public/fixtures.json",
    ],
    options: { typeAware: true },
  },
  fmt: {
    // These files use Stillwind's CSS registry DSL (`@utility`, `@variant`,
    // wildcard property patterns, and empty container wrappers). Formatting
    // them as standard CSS can change their semantics.
    ignorePatterns: ["packages/react/css/**"],
    sortImports: true,
    sortPackageJson: true,
  },
  run: {
    tasks: {
      fmt: {
        command: "vp fmt --check",
      },
      "fmt:write": {
        command: "vp fmt",
      },
      "webref:generate": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:generate"],
      },
      "webref:check": {
        command: "",
        dependsOn: ["@stillwind/tools#webref:check"],
      },
      lint: {
        command: "vp lint",
      },
      check: {
        command: "vp check",
      },
      "ts7-shadow:poc": {
        command: "node experiments/ts7-shadow-checker/src/index.mjs",
      },
      "fixtures:refresh": {
        command: "",
        dependsOn: ["@stillwind/tools#review:refresh"],
      },
      "fixtures:check": {
        command: "",
        dependsOn: ["@stillwind/tools#review:check"],
      },
      "tailwind:parity": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:parity"],
      },
      "tailwind:parity:all": {
        command: "",
        dependsOn: ["@stillwind/tools#tailwind:parity:all"],
      },
    },
  },
});
