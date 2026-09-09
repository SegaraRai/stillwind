import { resolve } from "node:path";

import { verifyPropertyOrderParity } from "./lib/property-order-parity.ts";

const root = resolve(import.meta.dirname, "..");
const result = verifyPropertyOrderParity(resolve(root, "fixtures", "tailwind-v4"), {
  fileFilter: process.env.STILLWIND_PARITY_FILE,
});

console.log(
  `Tailwind property-order parity: ${result.constraints} interfering-property constraints across ${result.candidates} candidates in ${result.cases} fixture cases.`,
);
