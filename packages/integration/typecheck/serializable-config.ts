import { withStillwind } from "../src/next/index.ts";
import { stillwindRules } from "../src/turbopack/rules.ts";

withStillwind({}, { configFile: "stillwind.config.ts", automatic: false });
stillwindRules({ entries: ["entry.tsx"], utilityOrder: "tailwind" });

// @ts-expect-error Loader workers require serializable inline settings.
withStillwind({}, { naming: { utilityClass: () => "custom" } });

// @ts-expect-error Loader workers require serializable inline settings.
stillwindRules({ entries: ["entry.tsx"], naming: { utilityClass: () => "custom" } });
