import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { defineConfig } from "vite-plus";
export default defineConfig({ run: { tasks: fixtureTasks("fresh", import.meta.dirname) } });
