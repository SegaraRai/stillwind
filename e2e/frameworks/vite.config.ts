import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    environment: "node",
    include: ["scripts/verify-source-map.test.ts"],
  },
  run: {
    tasks: {
      "build:packages": {
        command: "",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
      typecheck: {
        command: "tsc --noEmit",
      },
      test: {
        command: "vp test --run",
        cache: false,
      },
      browser: {
        command: "vp test --run --config browser.config.ts",
        dependsOn: ["build:packages", "typecheck", "test"],
        cache: false,
      },
      "browser:composition": {
        command: 'vp test --run --config browser.config.ts -t "^composition$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:emission": {
        command: 'vp test --run --config browser.config.ts -t "^emission$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:runtime-if": {
        command: 'vp test --run --config browser.config.ts -t "^runtime-if$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:conditional": {
        command: 'vp test --run --config browser.config.ts -t "^conditional$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:animation": {
        command: 'vp test --run --config browser.config.ts -t "^animation$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:view-transition": {
        command: 'vp test --run --config browser.config.ts -t "^view-transition$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:next-turbopack": {
        command: 'vp test --run --config browser.config.ts -t "^next-turbopack$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:next-standalone": {
        command: 'vp test --run --config browser.config.ts -t "^next-standalone$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:next-webpack": {
        command: 'vp test --run --config browser.config.ts -t "^next-webpack$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:rspack": {
        command: 'vp test --run --config browser.config.ts -t "^rspack$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:rsbuild": {
        command: 'vp test --run --config browser.config.ts -t "^rsbuild$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:react-router": {
        command: 'vp test --run --config browser.config.ts -t "^react-router$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:solidstart": {
        command: 'vp test --run --config browser.config.ts -t "^solidstart$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:tanstack-vite": {
        command: 'vp test --run --config browser.config.ts -t "^tanstack-vite$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:tanstack-rsbuild": {
        command: 'vp test --run --config browser.config.ts -t "^tanstack-rsbuild$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
      "browser:core": {
        command:
          'vp test --run --config browser.config.ts -t "^(animation|view-transition|composition|emission|conditional|runtime-if)$"',
        dependsOn: ["build:packages"],
        cache: false,
      },
    },
  },
});
