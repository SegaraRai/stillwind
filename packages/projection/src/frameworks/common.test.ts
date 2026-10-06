import { expect, test } from "vite-plus/test";

import { PreparationError } from "../preparation.ts";
import { sourceMapV3, sourceRegions } from "./common.ts";

test("invalid converter metadata produces a known projection failure with its identity", () => {
  const converter = {
    name: "converter",
    version: "1.0.0",
    optionsFingerprint: "options",
  };
  const operations = [
    () => sourceMapV3("{", converter),
    () =>
      sourceMapV3(
        {
          sources: [],
        },
        converter,
      ),
    () =>
      sourceRegions(
        "text",
        [
          {
            id: 0,
            start: 0,
            end: 10,
          },
        ],
        0,
        converter,
      ),
  ];
  for (const operation of operations) {
    expect(operation).toThrow(PreparationError);
    expect(operation).toThrow(
      expect.objectContaining({
        issue: expect.objectContaining({ kind: "projection" }),
        converter,
      }),
    );
  }
});
