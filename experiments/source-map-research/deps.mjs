import { createRequire } from "node:module";
const require = createRequire(new URL("../../packages/host/package.json", import.meta.url));
export const {
  GenMapping,
  addSegment,
  setSourceContent,
  toEncodedMap,
} = require("@jridgewell/gen-mapping");
export const {
  TraceMap,
  decodedMappings,
  originalPositionFor,
  generatedPositionFor,
} = require("@jridgewell/trace-mapping");
export const remapping = require("@jridgewell/remapping");
