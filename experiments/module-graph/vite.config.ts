import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      "build:packages": {
        command: "vp -C ../.. run --filter @stillwind/integration build",
        cache: false,
      },
      "verify:source-map": {
        command: "node --test scripts/verify-source-map.test.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      verify: {
        command: "node scripts/verify.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:stillwind": {
        command: "node scripts/verify-stillwind.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:composition": {
        command: "node scripts/verify-composition.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:emission": {
        command: "node scripts/verify-emission.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:runtime-if": {
        command: "node scripts/verify-runtime-if.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:conditional": {
        command: "node scripts/verify-conditional.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:animation": {
        command: "node scripts/verify-animation.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:view-transition": {
        command: "node scripts/verify-view-transition.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:adapters": {
        command: "node scripts/verify-adapters.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:turbopack": {
        command: "node scripts/verify-adapters.mjs turbopack standalone",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:webpack": {
        command: "node scripts/verify-adapters.mjs webpack",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:rspack": {
        command: "node scripts/verify-rspack.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:rsbuild": {
        command: "node scripts/verify-rspack.mjs rsbuild",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:react-router": {
        command: "node scripts/verify-react-router.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:solidstart": {
        command: "node scripts/verify-solidstart.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:tanstack-vite": {
        command: "node scripts/verify-tanstack.mjs",
        dependsOn: ["build:packages"],
        cache: false,
      },
      "verify:tanstack-rsbuild": {
        command: "node scripts/verify-tanstack.mjs rsbuild",
        dependsOn: ["build:packages"],
        cache: false,
      },
    },
  },
});
