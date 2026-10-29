import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "sites",
    include: ["*.test.ts"],
  },
  run: {
    tasks: {
      "check:urls": {
        command: "node check-urls.ts",
        cache: {
          env: ["PUBLIC_PLAYGROUND_URL", "PUBLIC_WEBSITE_URL"],
          input: [{ auto: true }, "../../apps/play/dist/site/**", "../../apps/website/dist/**"],
        },
        dependsOn: [
          "@stillwind-internal/apps-play#build",
          "@stillwind-internal/apps-website#build",
        ],
      },
      "prepare:play": {
        command: "node cloudflare.ts play",
        cache: {
          env: ["SITE_DEPLOYMENT"],
          input: [
            { auto: true },
            "../../apps/play/dist/site/**",
            "../../apps/play/cloudflare.config.ts",
            { pattern: "!.deploy", base: "workspace" },
            { pattern: "!.deploy/**", base: "workspace" },
          ],
          output: ["../../.deploy/play/**"],
        },
        dependsOn: ["@stillwind-internal/apps-play#build"],
      },
      "prepare:website": {
        command: "node cloudflare.ts website",
        cache: {
          env: ["SITE_DEPLOYMENT"],
          input: [
            { auto: true },
            "../../apps/website/dist/**",
            "../../apps/website/cloudflare.config.ts",
            { pattern: "!.deploy", base: "workspace" },
            { pattern: "!.deploy/**", base: "workspace" },
          ],
          output: ["../../.deploy/website/**"],
        },
        dependsOn: ["@stillwind-internal/apps-website#build"],
      },
      "check:production": {
        command: "",
        dependsOn: ["check:production:website", "check:production:play"],
      },
      "check:production:play": {
        command: "node deploy.ts play stable --dry-run",
        cache: false,
        dependsOn: ["prepare:play"],
      },
      "check:production:website": {
        command: "node deploy.ts website stable --dry-run",
        cache: false,
        dependsOn: ["prepare:website"],
      },
      deploy: {
        command: "node deploy.ts",
        cache: false,
      },
      typecheck: {
        command: "tsc --noEmit",
      },
      test: {
        command: "vp test --run",
        cache: testCache,
      },
    },
  },
});
