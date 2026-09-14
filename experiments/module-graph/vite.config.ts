import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      "verify:source-map": {
        command: "node --test scripts/verify-source-map.test.mjs",
        cache: false,
      },
      verify: { command: "node scripts/verify.mjs", cache: false },
      "verify:stillwind": { command: "node scripts/verify-stillwind.mjs", cache: false },
      "verify:composition": { command: "node scripts/verify-composition.mjs", cache: false },
      "verify:runtime-if": { command: "node scripts/verify-runtime-if.mjs", cache: false },
      "verify:conditional": { command: "node scripts/verify-conditional.mjs", cache: false },
      "verify:animation": {
        command: "node scripts/verify-animation.mjs",
        cache: false,
      },
      "verify:adapters": { command: "node scripts/verify-adapters.mjs", cache: false },
      "verify:webpack": { command: "node scripts/verify-adapters.mjs webpack", cache: false },
      "verify:rspack": { command: "node scripts/verify-rspack.mjs", cache: false },
      "verify:rsbuild": { command: "node scripts/verify-rspack.mjs rsbuild", cache: false },
      "verify:react-router": { command: "node scripts/verify-react-router.mjs", cache: false },
      "verify:solidstart": { command: "node scripts/verify-solidstart.mjs", cache: false },
      "verify:tanstack-vite": { command: "node scripts/verify-tanstack.mjs", cache: false },
      "verify:tanstack-rsbuild": {
        command: "node scripts/verify-tanstack.mjs rsbuild",
        cache: false,
      },
    },
  },
});
