import { lintPresets } from "@stillwind/config";

import { withStillwind } from "../src/next/index.ts";
import { stillwindRules } from "../src/turbopack/rules.ts";

withStillwind({}, { configFile: "stillwind.config.ts", automatic: false });
stillwindRules({ entries: ["entry.tsx"], utilityOrder: "tailwind" });
withStillwind({}, { lint: { extends: [lintPresets.strict] } });
stillwindRules({ entries: ["entry.tsx"], lint: { extends: [lintPresets.recommended] } });

// @ts-expect-error Loader workers require serializable inline settings.
withStillwind({}, { naming: { utilityClass: () => "custom" } });

// @ts-expect-error Loader workers require serializable inline settings.
stillwindRules({ entries: ["entry.tsx"], naming: { utilityClass: () => "custom" } });
