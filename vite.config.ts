/** Vite+ task configuration shared by the Stillwind workspace. */
export default {
  lint: {
    ignorePatterns: [
      "fixtures/projects/*/expected/**",
      "apps/fixtures-viewer/public/fixtures.json",
    ],
    options: { typeAware: true },
  },
  fmt: {
    ignorePatterns: [],
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
        command: "node tools/generate-webref-css-domains.mts",
      },
      "webref:check": {
        command: "node tools/generate-webref-css-domains.mts --check",
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
        command: "node tools/review-fixtures.mts",
        dependsOn: ["@stillwind/native#build"],
      },
      "fixtures:check": {
        command: "node tools/review-fixtures.mts --check",
        dependsOn: ["@stillwind/native#build"],
      },
    },
  },
};
