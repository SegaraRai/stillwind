import process from "node:process";

import { startLanguageServer } from "@stillwind/language-server/node";

startLanguageServer(process.argv[2]);
