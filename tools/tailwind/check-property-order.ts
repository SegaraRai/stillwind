import { fileURLToPath } from "node:url";

import { verifyPropertyOrderParity } from "./property-order-fixtures.ts";

const result = verifyPropertyOrderParity(
  fileURLToPath(new URL("../../fixtures/tailwind-v4/", import.meta.url)),
  { fileFilter: process.env.STILLWIND_PARITY_FILE },
);
console.log(
  `Tailwind property-order parity: ${result.constraints} constraints across ${result.candidates} candidates in ${result.cases} cases.`,
);
