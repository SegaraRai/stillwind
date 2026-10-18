import config from "@stillwind-internal/e2e-harness/acceptance-config";
import { defineConfig, mergeConfig } from "vite-plus";

export default mergeConfig(
  config,
  defineConfig({ root: import.meta.dirname, test: { name: "tailwind-plugins" } }),
);
