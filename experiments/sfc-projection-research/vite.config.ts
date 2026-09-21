import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      research: {
        command: "node research.mjs",
        cache: false,
      },
      verify: {
        command: "node verify.mjs",
        dependsOn: ["research"],
        cache: false,
      },
      "verify-native": {
        command: "node verify-native.mjs",
        cache: false,
      },
      "verify-normalization": {
        command: "node verify-normalization.mjs",
        cache: false,
      },
      "compare-checkers": {
        command: "node checker-comparison.mjs",
        dependsOn: ["@stillwind/projection#build", "stillwind#build"],
        cache: false,
      },
    },
  },
});
