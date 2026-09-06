import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      test: {
        command: "node --test test/type-facts.test.mts",
        dependsOn: ["@stillwind/native#build"],
      },
    },
  },
});
