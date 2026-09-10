import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      verify: { command: "node scripts/verify.mjs", cache: false },
      "verify:stillwind": { command: "node scripts/verify-stillwind.mjs", cache: false },
    },
  },
});
