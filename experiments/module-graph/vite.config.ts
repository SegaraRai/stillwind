import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      verify: { command: "node scripts/verify.mjs", cache: false },
      "verify:stillwind": { command: "node scripts/verify-stillwind.mjs", cache: false },
      "verify:adapters": { command: "node scripts/verify-adapters.mjs", cache: false },
      "verify:webpack": { command: "node scripts/verify-adapters.mjs webpack", cache: false },
      "verify:rspack": { command: "node scripts/verify-rspack.mjs", cache: false },
      "verify:rsbuild": { command: "node scripts/verify-rspack.mjs rsbuild", cache: false },
      "verify:react-router": { command: "node scripts/verify-react-router.mjs", cache: false },
      "verify:solidstart": { command: "node scripts/verify-solidstart.mjs", cache: false },
    },
  },
});
