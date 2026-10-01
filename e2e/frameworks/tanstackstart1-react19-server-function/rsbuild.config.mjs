import path from "node:path";

import { pluginReact } from "@rsbuild/plugin-react";
import * as bundler from "@stillwind/integration/rsbuild";
import { tanstackStart } from "@tanstack/react-start/plugin/rsbuild";

export default {
  root: process.cwd(),
  plugins: [
    bundler.pluginStillwind({
      css: path.join(process.cwd(), "root.css"),
      root: process.cwd(),
    }),
    pluginReact(),
    tanstackStart(),
  ],
  server: {
    port: Number(process.env.STILLWIND_E2E_PORT || 5173),
    strictPort: true,
  },
  output: {
    sourceMap: {
      js: "source-map",
      css: true,
    },
  },
  tools: {
    rspack: {
      cache: {
        type: "persistent",
        storage: {
          type: "filesystem",
          directory: path.join(process.cwd(), "cache"),
        },
      },
    },
  },
};
