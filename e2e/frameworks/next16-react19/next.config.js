import { existsSync } from "node:fs";
import { dirname, join } from "node:path";

import { withStillwind } from "@stillwind/integration/next";
import { stillwindRules } from "@stillwind/integration/turbopack";

const root = process.cwd();
let workspace = root;
while (!existsSync(join(workspace, "pnpm-workspace.yaml")) && dirname(workspace) !== workspace) {
  workspace = dirname(workspace);
}

const options = {
  css: "root.css",
  root,
};

export default process.env.STILLWIND_E2E_STANDALONE === "1"
  ? {
      agentRules: false,
      productionBrowserSourceMaps: true,
      turbopack: {
        root: workspace,
        rules: stillwindRules({
          ...options,
          entries: ["app/layout.jsx", "app/page.jsx", "app/lazy/page.jsx"],
        }),
      },
    }
  : withStillwind(
      {
        productionBrowserSourceMaps: true,
        turbopack: {
          root: workspace,
        },
      },
      options,
    );
