import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      research: { command: "node research.mjs", cache: false },
      verify: { command: "node verify.mjs", dependsOn: ["research"], cache: false },
    },
  },
});
