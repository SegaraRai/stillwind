import { parseArgs } from "node:util";

import { startLanguageServer } from "@stillwind/language-server/node";

// Lenient parsing accepts the options that the language client adds, such as `--stdio`.
const { config } = parseArgs({
  options: {
    config: {
      type: "string",
    },
  },
  strict: false,
  allowPositionals: true,
}).values;

startLanguageServer(typeof config === "string" ? config : undefined);
