import type { PreparedSource as ProjectionPreparedSource } from "@stillwind/projection";

import type { TypeFact } from "../semantics/index.ts";

/**
 * Prepared direct or projected source with optional semantic facts in original UTF-8 coordinates.
 * The host attaches these facts before native compilation.
 */
export type PreparedSource = ProjectionPreparedSource & {
  typeFacts?: TypeFact[];
};
